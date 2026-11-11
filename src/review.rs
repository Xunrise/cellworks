//! A pull request opened as a review: the panel's own state machine (the
//! list, a row checked out and mapped, the review itself) and the v13 panel
//! that draws it (STORY-022).
//!
//! Driven the same way `crate::opened::OpenedProject` drives an ordinary
//! read: [`ReviewSession::drain`] is called once a frame (never blocking),
//! advances the state and hands back a [`ReviewAction`] the binary acts on
//! (opening the head as a project, which reuses the ordinary opening path so
//! `OpenedProject`/`CodeAnalysisEvent` are unchanged, STORY-022 decision
//! "the head is kept as a checkout and opened as a project"). [`panel`] draws
//! the right-hand panel and hands back a [`PanelAction`] the binary acts on
//! (picking a row, picking a tower, leaving).
//!
//! The overlay itself is built here, not in Map or Sources, since it is the
//! binary's own join of the two (`docs/arc42/05-building-blocks.md`): Sources
//! produces [`cellworks_sources::pull_requests::FilePlacement`]s, Map owns
//! [`cellworks_map::ReviewOverlay`]; this module turns one into the other
//! ([`overlay_from`]) and groups the changed towers by file for the panel
//! ([`groups`]).

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};

use cellworks_code_analysis::{CodeAnalysisEvent, GitListing};
use cellworks_map::{ProjectMap, ReviewOverlay, default_prefix_rule, prefix_rule};
use cellworks_sources::pull_requests::forge::{Checkout, Forge};
use cellworks_sources::pull_requests::{
    Diff, FilePlacement, HeadCheckout, PullRequest, PullRequestEvent, PullRequestState, Size,
    TowerPlacement, place, start_check_out, start_list,
};
use cellworks_sources::stories::{StoryEvent, StoryText, start_read_with};
use cellworks_workspace::Project;

/// The fixed width of the review panel, the right column beside the city,
/// mirroring `src/app.rs`'s own `WHY_WIDTH`.
pub const REVIEW_WIDTH: f32 = 420.0;

/// What advancing a [`ReviewSession`] asks the binary to do: things that
/// reach past this module, the same way `cellworks_navigation::Moved` does
/// for the ordinary layers.
pub enum ReviewAction {
    /// The pull request's head was checked out: open it as an ordinary
    /// project named for the review, then enter review mode for `number`
    /// (in that order: opening a project clears any previous review).
    OpenHead { project: Project, number: u32 },
}

/// What picking something in the panel asks the binary to do.
pub enum PanelAction {
    /// A list row was picked: start checking it out.
    Pick(PullRequest),
    /// A changed or impacted tower's row was picked: move the view to it.
    PickTower(PathBuf),
    /// The back control, or the "Leave review" button, was picked.
    Leave,
}

/// Which tab of the list is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    Open,
    Merged,
}

/// The list of pull requests, loading or loaded.
struct Listing {
    tab: Tab,
    receiver: Option<mpsc::Receiver<PullRequestEvent>>,
    open: Vec<PullRequest>,
    merged: Vec<PullRequest>,
    error: Option<String>,
}

impl Listing {
    fn empty() -> Listing {
        Listing {
            tab: Tab::Open,
            receiver: None,
            open: Vec::new(),
            merged: Vec::new(),
            error: None,
        }
    }

    fn failed(reason: String) -> Listing {
        Listing {
            error: Some(reason),
            ..Listing::empty()
        }
    }
}

/// A story's text, read in the background once a pull request naming one is
/// picked: `text` is `None` until it arrives, and stays `None` when
/// `receiver` has gone (the issue was not found, or the forge failed) so the
/// panel can tell "still reading" from "resolved, nothing found".
struct Story {
    receiver: Option<mpsc::Receiver<StoryEvent>>,
    text: Option<StoryText>,
}

impl Story {
    fn start(repository: PathBuf, id: String, forge: Arc<dyn Forge + Send + Sync>) -> Story {
        Story {
            receiver: Some(start_read_with(repository, vec![id], forge)),
            text: None,
        }
    }
}

/// What checking a pull request's head out yielded, kept for the life of the
/// review: its own `HeadCheckout` guard removes the worktree on a background
/// thread once dropped (STORY-022 decision, "the head is kept as a
/// checkout").
struct Checked {
    pull_request: PullRequest,
    /// Kept alive for as long as the review is: never read, its own `Drop`
    /// is the point of holding it (removes the worktree on a background
    /// thread once the review ends).
    _checkout: HeadCheckout,
    head_root: PathBuf,
    diff: Diff,
    size: Size,
    flags: BTreeMap<String, Vec<String>>,
}

/// How far opening a chosen pull request has got.
enum ReviewState {
    /// The panel lists pull requests; nothing picked yet.
    Listing(Listing),
    /// A row was picked: its head is being fetched, diffed and checked out.
    CheckingOut {
        pull_request: PullRequest,
        receiver: mpsc::Receiver<(PullRequestEvent, Option<HeadCheckout>)>,
        story: Option<Story>,
    },
    /// The head is checked out and opened as a project; this module's own
    /// copy of it is being mapped, off the UI thread, so [`place`] (pure CPU)
    /// can run over it once, on the UI thread (requirement 10).
    Mapping {
        checked: Checked,
        story: Option<Story>,
        map_receiver: mpsc::Receiver<Option<ProjectMap>>,
    },
    /// The review is shown: the overlay and the changed towers' groups are
    /// ready.
    Active {
        checked: Checked,
        story: Option<Story>,
        overlay: ReviewOverlay,
        groups: Vec<FileGroup>,
        /// Files whose "+ N more towers" was clicked, by their own path.
        unfolded: HashSet<String>,
    },
}

/// One tower of a [`FileGroup`], named and counted for the panel.
pub struct TowerRow {
    pub tower: PathBuf,
    pub name: String,
    pub added: usize,
    pub removed: usize,
}

/// The changed towers of one file, most lines first; `outside` is set when
/// the file holds lines no tower held (requirement 6's "no towers: lines
/// outside any tower").
pub struct FileGroup {
    pub file: String,
    pub towers: Vec<TowerRow>,
    pub outside: Option<(usize, usize)>,
}

impl FileGroup {
    fn total_lines(&self) -> usize {
        let towers: usize = self
            .towers
            .iter()
            .map(|tower| tower.added + tower.removed)
            .sum();
        let outside = self.outside.map_or(0, |(added, removed)| added + removed);
        towers + outside
    }
}

/// The review in progress: the project it is reviewing pull requests of (also
/// the one reopened on leaving, STORY-022 decision "leaving reopens the home
/// project"), and how far opening the picked one has got.
pub struct ReviewSession {
    home: Project,
    state: ReviewState,
}

impl ReviewSession {
    /// Starts listing `home`'s pull requests with `forge`; `None` for a
    /// remote project, which has no local folder to run `gh` or `git` in.
    pub fn start(home: Project, forge: Arc<dyn Forge + Send + Sync>) -> Option<ReviewSession> {
        let repo = home.id.local_path()?.to_path_buf();
        let receiver = start_list(repo, forge);
        Some(ReviewSession {
            home,
            state: ReviewState::Listing(Listing {
                tab: Tab::Open,
                receiver: Some(receiver),
                open: Vec::new(),
                merged: Vec::new(),
                error: None,
            }),
        })
    }

    /// Whether the panel is still just the list: pressing `p` again closes
    /// it. Once a row is picked, mid-checkout, mapping or shown as the
    /// review, `p` is a no-op (only Esc, the back control or the button end
    /// it, requirement 8).
    pub fn is_listing_only(&self) -> bool {
        matches!(self.state, ReviewState::Listing(_))
    }

    /// The overlay to draw, once the review is shown.
    pub fn overlay(&self) -> Option<&ReviewOverlay> {
        match &self.state {
            ReviewState::Active { overlay, .. } => Some(overlay),
            _ => None,
        }
    }

    /// Picks `pull_request` from the list: starts checking its head out
    /// (`checkout`) and, when it names a story, reading that story's text
    /// (`forge`) in the background, independently. Nothing while a row was
    /// already picked.
    pub fn pick(
        &mut self,
        pull_request: PullRequest,
        checkout: Arc<dyn Checkout + Send + Sync>,
        forge: Arc<dyn Forge + Send + Sync>,
    ) {
        if !matches!(self.state, ReviewState::Listing(_)) {
            return;
        }
        let Some(repo) = self.home.id.local_path().map(Path::to_path_buf) else {
            return;
        };
        let story = pull_request
            .story
            .clone()
            .map(|id| Story::start(repo.clone(), id, forge));
        let receiver = start_check_out(repo, pull_request.clone(), checkout);
        self.state = ReviewState::CheckingOut {
            pull_request,
            receiver,
            story,
        };
    }

    /// Takes whatever the list, the checkout, the head mapping and the story
    /// have sent so far, and advances the state. Never waits. `git` maps the
    /// checkout the same way an ordinary project is read, so a test's own
    /// fake git answers for the review's head too, not just its home
    /// (`crate::app::CellworksApp::read`'s own convention); `ignored` mirrors
    /// the engineer's current choice for ignored files.
    pub fn drain(
        &mut self,
        git: &Arc<dyn GitListing + Send + Sync>,
        ignored: bool,
    ) -> Option<ReviewAction> {
        let state = std::mem::replace(&mut self.state, ReviewState::Listing(Listing::empty()));
        let (state, action) = advance(state, &self.home, git, ignored);
        self.state = state;
        action
    }

    /// The project open before the review started, to reopen on leaving.
    pub fn into_home(self) -> Project {
        self.home
    }
}

/// Advances `state` by one drain: polls whichever channels it holds and
/// moves to the next state when one completes. Taking `state` by value (its
/// caller hands it over with `std::mem::replace`) is what lets a transition
/// move a non-`Clone` field (a receiver, a `HeadCheckout`) into the next
/// variant without borrow trouble.
fn advance(
    state: ReviewState,
    home: &Project,
    git: &Arc<dyn GitListing + Send + Sync>,
    ignored: bool,
) -> (ReviewState, Option<ReviewAction>) {
    match state {
        ReviewState::Listing(mut listing) => {
            poll_listing(&mut listing);
            (ReviewState::Listing(listing), None)
        }
        ReviewState::CheckingOut {
            pull_request,
            receiver,
            mut story,
        } => {
            if let Some(story) = &mut story {
                poll_story(story);
            }
            match receiver.try_recv() {
                Ok((
                    PullRequestEvent::CheckedOut {
                        head,
                        diff,
                        size,
                        flags,
                        ..
                    },
                    Some(checkout),
                )) => {
                    let number = pull_request.number;
                    let mut head_project = Project::at(head.clone());
                    head_project.name = format!("{} · review #{number}", home.name);
                    head_project.district_prefix = home.district_prefix.clone();
                    let rule = head_project
                        .district_prefix
                        .as_deref()
                        .and_then(prefix_rule)
                        .unwrap_or_else(default_prefix_rule);
                    let map_receiver = start_map_head(
                        head.clone(),
                        head_project.name.clone(),
                        rule,
                        Arc::clone(git),
                        ignored,
                    );
                    let checked = Checked {
                        pull_request,
                        _checkout: checkout,
                        head_root: head,
                        diff,
                        size,
                        flags,
                    };
                    (
                        ReviewState::Mapping {
                            checked,
                            story,
                            map_receiver,
                        },
                        Some(ReviewAction::OpenHead {
                            project: head_project,
                            number,
                        }),
                    )
                }
                // Dropping a `HeadCheckout` here, if there somehow were one,
                // would run its removal on a background thread anyway
                // (`HeadCheckout::drop`); `OpenFailed` never carries one.
                Ok((PullRequestEvent::OpenFailed { reason, .. }, _)) => {
                    (ReviewState::Listing(Listing::failed(reason)), None)
                }
                Ok(_) => (
                    ReviewState::CheckingOut {
                        pull_request,
                        receiver,
                        story,
                    },
                    None,
                ),
                Err(mpsc::TryRecvError::Empty) => (
                    ReviewState::CheckingOut {
                        pull_request,
                        receiver,
                        story,
                    },
                    None,
                ),
                Err(mpsc::TryRecvError::Disconnected) => (
                    ReviewState::Listing(Listing::failed(
                        "the checkout ended without a result".to_owned(),
                    )),
                    None,
                ),
            }
        }
        ReviewState::Mapping {
            checked,
            mut story,
            map_receiver,
        } => {
            if let Some(story) = &mut story {
                poll_story(story);
            }
            match map_receiver.try_recv() {
                Ok(Some(map)) => {
                    let placements = place(&checked.diff, &map, &checked.head_root);
                    let overlay = overlay_from(&placements, &map);
                    let groups = groups(&placements);
                    (
                        ReviewState::Active {
                            checked,
                            story,
                            overlay,
                            groups,
                            unfolded: HashSet::new(),
                        },
                        None,
                    )
                }
                Ok(None) | Err(mpsc::TryRecvError::Disconnected) => (
                    ReviewState::Listing(Listing::failed(
                        "the pull request's head could not be mapped".to_owned(),
                    )),
                    None,
                ),
                Err(mpsc::TryRecvError::Empty) => (
                    ReviewState::Mapping {
                        checked,
                        story,
                        map_receiver,
                    },
                    None,
                ),
            }
        }
        ReviewState::Active {
            checked,
            mut story,
            overlay,
            groups,
            unfolded,
        } => {
            if let Some(story) = &mut story {
                poll_story(story);
            }
            (
                ReviewState::Active {
                    checked,
                    story,
                    overlay,
                    groups,
                    unfolded,
                },
                None,
            )
        }
    }
}

/// Polls `listing`'s receiver, if it still has one.
fn poll_listing(listing: &mut Listing) {
    let Some(receiver) = &listing.receiver else {
        return;
    };
    match receiver.try_recv() {
        Ok(PullRequestEvent::Listed { pull_requests }) => {
            listing.open = pull_requests
                .iter()
                .filter(|pull_request| pull_request.state == PullRequestState::Open)
                .cloned()
                .collect();
            listing.merged = pull_requests
                .into_iter()
                .filter(|pull_request| pull_request.state == PullRequestState::Merged)
                .collect();
            listing.receiver = None;
        }
        Ok(PullRequestEvent::ListFailed { reason }) => {
            listing.error = Some(reason);
            listing.receiver = None;
        }
        Ok(_) => {}
        Err(mpsc::TryRecvError::Empty) => {}
        Err(mpsc::TryRecvError::Disconnected) => listing.receiver = None,
    }
}

/// Polls `story`'s receiver, if it still has one: found or not, one id is
/// read at most once, so the first answer (or the channel going quiet) ends
/// it.
fn poll_story(story: &mut Story) {
    let Some(receiver) = &story.receiver else {
        return;
    };
    match receiver.try_recv() {
        Ok(StoryEvent::Read { story: text, .. }) => {
            story.text = Some(text);
            story.receiver = None;
        }
        Ok(StoryEvent::ReadFailed { .. }) => story.receiver = None,
        Err(mpsc::TryRecvError::Empty) => {}
        Err(mpsc::TryRecvError::Disconnected) => story.receiver = None,
    }
}

/// Maps the checkout at `root` the way an ordinary project is read
/// (`crate::app::CellworksApp::read`), on a background thread, so [`place`]
/// has a `ProjectMap` to run over (requirement 10) built with the home
/// project's own district rule and git source, never the real `GitCli`
/// directly (so a test's fake git answers for it too).
fn start_map_head(
    root: PathBuf,
    name: String,
    prefix: &'static cellworks_map::PrefixRule,
    git: Arc<dyn GitListing + Send + Sync>,
    ignored: bool,
) -> mpsc::Receiver<Option<ProjectMap>> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let events = cellworks_code_analysis::load(&root, &name, prefix, git.as_ref(), ignored);
        let map = events.into_iter().find_map(|event| match event {
            CodeAnalysisEvent::ProjectMapped { map, .. } => Some(*map),
            _ => None,
        });
        let _ = sender.send(map);
    });
    receiver
}

/// Turns `placements` into the overlay `map` draws: every tower a
/// [`FilePlacement`] names gets the floor numbers it was written on and the
/// floor numbers lines were removed before; [`ReviewOverlay::new`] then walks
/// `map` for the towers that call one of them.
pub fn overlay_from(placements: &[FilePlacement], map: &ProjectMap) -> ReviewOverlay {
    let mut written: BTreeMap<PathBuf, Vec<usize>> = BTreeMap::new();
    let mut removed: BTreeMap<PathBuf, Vec<usize>> = BTreeMap::new();
    for placement in placements {
        for tower in &placement.towers {
            if !tower.added.is_empty() {
                written.insert(tower.tower.clone(), tower.added.clone());
            }
            if !tower.removed.is_empty() {
                removed.insert(
                    tower.tower.clone(),
                    tower
                        .removed
                        .iter()
                        .map(|removal| removal.before_new_line)
                        .collect(),
                );
            }
        }
    }
    ReviewOverlay::new(&written, &removed, map)
}

/// Whether the pull request added or removed any line of `tower`; a changed
/// file's untouched towers are placed too, but are not changed towers.
fn changed(tower: &TowerPlacement) -> bool {
    !tower.added.is_empty() || !tower.removed.is_empty()
}

/// Groups `placements` by file for the panel: within a file, the towers most
/// lines first (requirement 6); files themselves most lines first too. A file
/// whose touched lines fall outside every tower keeps them as `outside`
/// rather than dropping them (the "no towers: lines outside any tower" case);
/// a file with neither is left out.
pub fn groups(placements: &[FilePlacement]) -> Vec<FileGroup> {
    let mut groups: Vec<FileGroup> = placements
        .iter()
        .filter(|placement| {
            placement.towers.iter().any(changed)
                || placement.outside_added > 0
                || placement.outside_removed > 0
        })
        .map(|placement| {
            let mut towers: Vec<TowerRow> = placement
                .towers
                .iter()
                .filter(|tower| changed(tower))
                .map(|tower| TowerRow {
                    tower: tower.tower.clone(),
                    name: tower_name(&tower.tower),
                    added: tower.added.len(),
                    removed: tower.removed.len(),
                })
                .collect();
            towers.sort_by_key(|tower| std::cmp::Reverse(tower.added + tower.removed));
            let outside = if placement.outside_added > 0 || placement.outside_removed > 0 {
                Some((placement.outside_added, placement.outside_removed))
            } else {
                None
            };
            FileGroup {
                file: placement.file.clone(),
                towers,
                outside,
            }
        })
        .collect();
    groups.sort_by_key(|group| std::cmp::Reverse(group.total_lines()));
    groups
}

/// The tower's own name, its path's last component (a tower's path is its
/// file joined with its qualified name, `cellworks_map::graph::Item`'s own
/// doc comment).
fn tower_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Draws the review panel's content for the state `session` is in, and hands
/// back what was picked. `picked` is the tower currently marked in the
/// review's own list (`cellworks_navigation::ReviewMode::picked`), so its row
/// can be shown selected.
pub fn panel(
    ui: &mut egui::Ui,
    session: &mut ReviewSession,
    picked: Option<&Path>,
) -> Option<PanelAction> {
    match &mut session.state {
        ReviewState::Listing(listing) => panel_listing(ui, listing),
        ReviewState::CheckingOut { pull_request, .. } => {
            ui.label(format!("Checking out #{}…", pull_request.number));
            None
        }
        ReviewState::Mapping { .. } => {
            ui.label("Mapping the head…");
            None
        }
        ReviewState::Active {
            checked,
            story,
            overlay,
            groups,
            unfolded,
        } => panel_active(ui, checked, story, overlay, groups, unfolded, picked),
    }
}

/// The list state: the Open/Merged tabs and the rows of whichever is shown.
fn panel_listing(ui: &mut egui::Ui, listing: &mut Listing) -> Option<PanelAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        if ui
            .selectable_label(listing.tab == Tab::Open, "Open")
            .clicked()
        {
            listing.tab = Tab::Open;
        }
        if ui
            .selectable_label(listing.tab == Tab::Merged, "Merged")
            .clicked()
        {
            listing.tab = Tab::Merged;
        }
    });
    if let Some(error) = &listing.error {
        ui.label(error);
    }
    let rows: &[PullRequest] = match listing.tab {
        Tab::Open => &listing.open,
        Tab::Merged => &listing.merged,
    };
    for pull_request in rows {
        ui.horizontal(|ui| {
            let label = format!(
                "#{} · {}",
                pull_request.number,
                pull_request.story.as_deref().unwrap_or("NO STORY")
            );
            // A plain `Label`, not a `selectable_label`: its accesskit role
            // stays `Label`, so a test waiting on the list to load (which
            // reads only `Role::Label` nodes) sees it, while `.sense(click)`
            // still makes it the row's one clickable element.
            if ui
                .add(egui::Label::new(label).sense(egui::Sense::click()))
                .clicked()
            {
                action = Some(PanelAction::Pick(pull_request.clone()));
            }
            ui.label(format!("+{}", pull_request.additions));
            ui.label(format!("−{}", pull_request.deletions));
            ui.label(format!("{} files", pull_request.changed_files));
            for flag in pull_request.flags.keys() {
                ui.label(flag);
            }
            ui.label(&pull_request.author);
        });
    }
    action
}

/// The review state: the v13 panel (design requirement 6).
fn panel_active(
    ui: &mut egui::Ui,
    checked: &Checked,
    story: &Option<Story>,
    overlay: &ReviewOverlay,
    groups: &[FileGroup],
    unfolded: &mut HashSet<String>,
    picked: Option<&Path>,
) -> Option<PanelAction> {
    let mut action = None;
    let pull_request = &checked.pull_request;

    ui.horizontal(|ui| {
        if ui.button("←").clicked() {
            action = Some(PanelAction::Leave);
        }
        ui.label("Esc leaves");
    });
    ui.label(format!("REVIEW #{}", pull_request.number));
    ui.label(match pull_request.state {
        PullRequestState::Open => "OPEN",
        PullRequestState::Merged => "MERGED",
    });
    ui.label(pull_request.story.as_deref().unwrap_or("NO STORY"));
    ui.label(&pull_request.title);
    ui.horizontal(|ui| {
        ui.label(&pull_request.author);
        ui.label(&pull_request.branch);
        ui.label(pull_request.head.get(..7).unwrap_or(&pull_request.head));
    });

    if pull_request.story.is_some() {
        match story {
            Some(story) if story.text.is_some() => {
                let text = story.text.as_ref().expect("checked above");
                ui.label(&text.title);
                ui.label(format!("As a {}", text.as_a));
                ui.label(format!("I want {}", text.i_want));
                ui.label(format!("So that {}", text.so_that));
            }
            Some(story) if story.receiver.is_none() => {
                ui.label("The story's issue was not found.");
            }
            // Still reading: shown once it resolves, not before.
            _ => {}
        }
    }

    ui.horizontal(|ui| {
        ui.label(format!("+{} -{}", checked.size.added, checked.size.removed));
        let total = (checked.size.added + checked.size.removed).max(1) as f32;
        ui.add(egui::ProgressBar::new(checked.size.added as f32 / total).desired_width(80.0));
        ui.label(format!("{} files", checked.size.files));
    });

    let written_count = overlay
        .towers
        .values()
        .filter(|review| !review.written.is_empty())
        .count();
    let removed_count = overlay
        .towers
        .values()
        .filter(|review| !review.removed.is_empty())
        .count();
    let impacted_count = overlay
        .towers
        .values()
        .filter(|review| review.impacted)
        .count();
    ui.horizontal(|ui| {
        ui.label(format!("{written_count} towers written"));
        ui.label(format!("{removed_count} towers with removals"));
        ui.label(format!("{impacted_count} impacted towers"));
    });

    for (flag, files) in &checked.flags {
        ui.label(flag);
        for file in files {
            ui.label(file);
        }
    }

    for group in groups {
        let path = Path::new(&group.file);
        let filename = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| group.file.clone());
        let folder = path
            .parent()
            .map(|parent| parent.to_string_lossy().into_owned())
            .unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label(&filename);
            ui.weak(&folder);
        });
        let unfold_all = unfolded.contains(&group.file);
        let limit = if unfold_all {
            group.towers.len()
        } else {
            group.towers.len().min(4)
        };
        for tower in &group.towers[..limit] {
            let label = format!("{} (+{} -{})", tower.name, tower.added, tower.removed);
            let selected = picked == Some(tower.tower.as_path());
            if ui.selectable_label(selected, label).clicked() {
                action = Some(PanelAction::PickTower(tower.tower.clone()));
            }
        }
        if !unfold_all && group.towers.len() > limit {
            let more = group.towers.len() - limit;
            if ui.button(format!("+ {more} more towers")).clicked() {
                unfolded.insert(group.file.clone());
            }
        }
        if let Some((added, removed)) = group.outside {
            let prefix = if group.towers.is_empty() {
                "no towers: "
            } else {
                ""
            };
            ui.label(format!(
                "{prefix}lines outside any tower (+{added} -{removed})"
            ));
        }
    }

    let mut impacted: Vec<&PathBuf> = overlay
        .towers
        .iter()
        .filter(|(_, review)| review.impacted)
        .map(|(path, _)| path)
        .collect();
    impacted.sort();
    for path in impacted {
        let name = tower_name(path);
        let selected = picked == Some(path.as_path());
        if ui
            .selectable_label(selected, format!("{name} (impacted)"))
            .clicked()
        {
            action = Some(PanelAction::PickTower(path.clone()));
        }
    }

    if ui.button("Leave review").clicked() {
        action = Some(PanelAction::Leave);
    }

    action
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tower(name: &str, added: Vec<usize>) -> TowerPlacement {
        TowerPlacement {
            tower: PathBuf::from(name),
            added,
            removed: Vec::new(),
        }
    }

    #[test]
    fn groups_leaves_out_the_untouched_towers_of_a_changed_file() {
        let placements = vec![
            FilePlacement {
                file: "src/pricing.rs".into(),
                towers: vec![
                    tower("src/pricing.rs/discount", vec![3]),
                    tower("src/pricing.rs/helper", vec![]),
                ],
                outside_added: 0,
                outside_removed: 0,
            },
            FilePlacement {
                file: "src/other.rs".into(),
                towers: vec![tower("src/other.rs/untouched", vec![])],
                outside_added: 0,
                outside_removed: 0,
            },
        ];
        let groups = groups(&placements);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].outside, None);
        let rows: Vec<_> = groups[0]
            .towers
            .iter()
            .map(|row| row.tower.clone())
            .collect();
        assert_eq!(rows, vec![PathBuf::from("src/pricing.rs/discount")]);
    }

    #[test]
    fn groups_counts_lines_outside_the_towers_of_a_file_that_has_towers() {
        let placements = vec![FilePlacement {
            file: "src/pricing.rs".into(),
            towers: vec![tower("src/pricing.rs/discount", vec![])],
            outside_added: 1,
            outside_removed: 0,
        }];
        let groups = groups(&placements);
        assert_eq!(groups.len(), 1);
        assert!(groups[0].towers.is_empty());
        assert_eq!(groups[0].outside, Some((1, 0)));
    }
}
