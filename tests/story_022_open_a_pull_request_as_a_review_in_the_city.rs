//! STORY-022, open a pull request as a review in the city: the engineer lists
//! a project's open and merged pull requests, opens one to see its head as
//! the city, the floors it writes and the lines it removes stand out, towers
//! it does not touch recede, a tower calling a changed one is marked
//! impacted, the panel lists the changed towers by file, and leaving the
//! review restores the project's own city.
//!
//! One test per scenario of issue #44, named after it. The project "shop" is
//! `common::review_home_tree`: two real crates in a temporary directory,
//! "catalog" ("discount", "total") and "orders" ("checkout", which calls
//! "catalog::discount", and "ship", touched by nothing). Pull request "#41"
//! implements "STORY-012", changes "discount" and "total" and is listed and
//! checked out by `common::FakeForge`/`common::FakeCheckout`, which never run
//! `gh` or `git` or touch the network; its head is `common::review_head_tree`.
//! No test opens a window.

mod common;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cellworks::app::CellworksApp;
use cellworks::opened::ProjectState;
use cellworks_map::ui::DrawnTower;
use egui::Key;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

use common::{FakeCheckout, FakeForge, REVIEW_DIFF, Recorded, Tree, review_home_tree};

/// Opens "shop" (`common::review_home_tree`) with `forge` and `checkout`
/// wired in, so no test ever reaches real `gh` or `git`.
fn open_shop(
    forge: Arc<FakeForge>,
    checkout: Arc<FakeCheckout>,
) -> (Tree, Harness<'static, CellworksApp>) {
    let tree = review_home_tree();
    let project = tree.as_project(None);
    let name = project.name.clone();
    let app = common::app(vec![project], Recorded::listing(&[]))
        .with_forge(forge)
        .with_checkout(checkout);
    let mut harness = common::harness(app);
    common::double_click(&mut harness, &name);
    common::await_read(&mut harness);
    (tree, harness)
}

/// `p` opens the review list panel, and this waits for the background list
/// (over `FakeForge`) to arrive: a label containing `needle` appears.
fn await_labelled(harness: &mut Harness<'_, CellworksApp>, needle: &str) {
    let started = Instant::now();
    loop {
        harness.state_mut().apply_pending();
        harness.run_steps(5);
        if common::labels(harness).iter().any(|l| l.contains(needle)) {
            return;
        }
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "never saw a label containing {needle:?}: {:?}",
            common::labels(harness)
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Opens the review list panel and waits for pull request "#41" to be listed.
fn open_panel(harness: &mut Harness<'_, CellworksApp>) {
    harness.key_press(Key::P);
    harness.run_steps(5);
    await_labelled(harness, "#41");
}

/// The breadcrumb above the map.
fn breadcrumb(harness: &Harness<'_, CellworksApp>) -> String {
    harness
        .root()
        .children_recursive()
        .map(|node| node.accesskit_node())
        .filter(|node| node.role() == Role::Label)
        .filter_map(|node| node.value().map(|text| text.to_string()))
        .find(|text| text.starts_with(common::BREADCRUMB))
        .expect("a breadcrumb")
}

/// Waits for the review's own head project to be mapped and the breadcrumb to
/// contain `needle` (checking out and mapping the head takes a background
/// thread each, STORY-022 requirement 10).
fn await_breadcrumb(harness: &mut Harness<'_, CellworksApp>, needle: &str) {
    let started = Instant::now();
    loop {
        harness.state_mut().apply_pending();
        harness.run_steps(5);
        let mapped = harness
            .state()
            .opened()
            .is_some_and(|opened| matches!(opened.state(), ProjectState::Mapped(_)));
        if mapped && breadcrumb(harness).contains(needle) {
            return;
        }
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "the review never finished opening: breadcrumb {:?}",
            breadcrumb(harness)
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// "shop" open as a review of pull request "#41".
struct Review {
    tree: Tree,
    harness: Harness<'static, CellworksApp>,
    checkout: Arc<FakeCheckout>,
}

/// The engineer opens the pull request list and chooses "#41".
fn open_review() -> Review {
    let forge = Arc::new(FakeForge::new());
    let checkout = Arc::new(FakeCheckout::new(REVIEW_DIFF));
    let (tree, mut harness) = open_shop(Arc::clone(&forge), Arc::clone(&checkout));
    open_panel(&mut harness);

    harness.get_by_label("#41 · STORY-012").click_accesskit();
    harness.run_steps(5);
    await_breadcrumb(&mut harness, "review #41");

    Review {
        tree,
        harness,
        checkout,
    }
}

impl Review {
    /// The root of the pull request's head, the folder the review opened.
    fn head_root(&self) -> PathBuf {
        common::map(&self.harness).root.clone()
    }

    /// The map path of the item `name` of `file`, at the head.
    fn head_item(&self, file: &str, name: &str) -> PathBuf {
        self.head_root().join(file).join(name)
    }

    /// The tower `name` of `file`, as last drawn.
    fn tower(&self, file: &str, name: &str) -> DrawnTower {
        common::tower(&self.harness, &self.head_item(file, name)).clone()
    }
}

/// Open and merged pull requests are listed with their story, size and flags
#[test]
fn open_and_merged_pull_requests_are_listed_with_their_story_size_and_flags() {
    let forge = Arc::new(FakeForge::new());
    let checkout = Arc::new(FakeCheckout::new(REVIEW_DIFF));
    let (_tree, mut harness) = open_shop(forge, checkout);

    open_panel(&mut harness);

    // Open is selected by default, and "#41" is listed on it, by its own
    // exact accessible label.
    harness.get_by_label("#41 · STORY-012");
    let found = common::labels(&harness);
    assert!(found.iter().any(|l| l.contains("+3")), "{found:?}");
    assert!(
        found.iter().any(|l| l.contains("−2") || l.contains("-2")),
        "{found:?}"
    );
    assert!(found.iter().any(|l| l.contains("2 files")), "{found:?}");
    assert!(
        found.iter().any(|l| l.contains("dependencies")),
        "{found:?}"
    );

    // The Merged tab holds the merged pull request, with no story.
    harness.get_by_label("Merged").click();
    harness.run_steps(5);
    await_labelled(&mut harness, "#9");
    harness.get_by_label("#9 · NO STORY");
}

/// A pull request opened from the list is shown as a review
#[test]
fn a_pull_request_opened_from_the_list_is_shown_as_a_review() {
    let review = open_review();

    assert_eq!(
        breadcrumb(&review.harness),
        "Universe › shop · review #41",
        "the city shows #41 at its head, named as its review"
    );
    assert_ne!(
        review.head_root(),
        review.tree.root,
        "the opened folder is the head's, not the home project's"
    );

    let found = common::labels(&review.harness);
    assert!(
        found
            .iter()
            .any(|l| l.contains("Give engineers a price breakdown")),
        "the story's title is shown: {found:?}"
    );
    assert!(
        found.iter().any(|l| l.contains("shopper")),
        "the story's \"As a\" is shown: {found:?}"
    );
    assert!(
        found.iter().any(|l| l.contains("discount and the total")),
        "the story's \"I want\" is shown: {found:?}"
    );
    assert!(
        found.iter().any(|l| l.contains("what I am really paying")),
        "the story's \"So that\" is shown: {found:?}"
    );
}

/// The floors a pull request writes stand out
#[test]
fn the_floors_a_pull_request_writes_stand_out() {
    let review = open_review();

    let discount = review.tower("catalog/src/pricing.rs", "discount");
    let info = discount
        .review
        .expect("discount was changed by the pull request");
    assert_eq!(
        info.written,
        vec![3, 4],
        "the two new lines of discount's own body"
    );
    assert!(info.pulsing, "a changed tower pulses gently");

    let ship = review.tower("orders/src/lib.rs", "ship");
    assert!(
        ship.review.is_none(),
        "ship is touched by nothing the pull request did"
    );
    assert!(
        ship.light < 0.3,
        "an untouched tower recedes, light was {}",
        ship.light
    );
}

/// Removed lines are shown where they were removed
#[test]
fn removed_lines_are_shown_where_they_were_removed() {
    let review = open_review();

    let total = review.tower("catalog/src/pricing.rs", "total");
    let info = total.review.expect("total was changed by the pull request");
    assert_eq!(info.removed.len(), 2, "two lines were removed from total");
    for floor in info.removed {
        assert!(
            (8..=14).contains(&floor),
            "removed floor {floor} is outside total's own range"
        );
    }
}

/// Callers of changed towers are marked as impacted
#[test]
fn callers_of_changed_towers_are_marked_as_impacted() {
    let review = open_review();

    let checkout = review.tower("orders/src/lib.rs", "checkout");
    let info = checkout
        .review
        .expect("checkout calls a tower the pull request changed");
    assert!(
        info.impacted,
        "checkout calls discount, which the pull request changed"
    );
}

/// The review lists the changed towers by file
#[test]
fn the_review_lists_the_changed_towers_by_file() {
    let mut review = open_review();

    let found = common::labels(&review.harness);
    assert!(
        found.iter().any(|l| l.contains("pricing.rs")),
        "the changed towers are grouped under the file that holds them: {found:?}"
    );
    review.harness.get_by_label("discount (+2 -0)");
    review.harness.get_by_label("total (+0 -2)");

    review
        .harness
        .get_by_label("discount (+2 -0)")
        .click_accesskit();
    review.harness.run_steps(5);

    let discount = review.head_item("catalog/src/pricing.rs", "discount");
    assert_eq!(
        review.harness.state().navigation().selection(),
        Some(discount.as_path()),
        "choosing discount in the list moves the view to the tower discount"
    );
}

/// How the engineer leaves the review, for
/// [`leaving_the_review_restores_the_city`].
enum Leave {
    Esc,
    Button,
}

/// Leaving the review restores the city
#[test]
fn leaving_the_review_restores_the_city() {
    for leave in [Leave::Esc, Leave::Button] {
        let mut review = open_review();
        assert!(breadcrumb(&review.harness).contains("review #41"));

        match leave {
            Leave::Esc => {
                review.harness.key_press(Key::Escape);
                review.harness.run_steps(5);
            }
            Leave::Button => {
                review.harness.get_by_label("Leave review").click();
                review.harness.run_steps(5);
            }
        }
        common::await_read(&mut review.harness);

        assert_eq!(
            breadcrumb(&review.harness),
            "Universe › shop",
            "the project's own city is shown again"
        );
        for tower in review.harness.state().towers() {
            assert!(
                tower.review.is_none(),
                "{} is still marked by the review",
                tower.path.display()
            );
            assert!(
                tower.light > 0.5,
                "{} still recedes, light was {}",
                tower.path.display(),
                tower.light
            );
        }

        let started = Instant::now();
        while review.checkout.removed_worktrees().is_empty()
            && started.elapsed() < Duration::from_secs(5)
        {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            review.checkout.removed_worktrees().len(),
            1,
            "the head worktree was removed on leaving"
        );
    }
}

/// Regression test for a counter-review finding: opening a different project
/// while a review session is still listing, or mid-checkout, must end that
/// session rather than let it render over (or, once its background checkout
/// completes, silently swap in) a project the engineer never asked to review.
/// At most one review is open at a time (design decision, STORY-022;
/// `docs/domain/contexts/Navigation/Navigation.md`).
#[test]
fn opening_a_different_project_ends_a_review_session_in_progress() {
    let forge = Arc::new(FakeForge::new());
    let checkout = Arc::new(FakeCheckout::new(REVIEW_DIFF));
    let shop = review_home_tree();
    let other = Tree::project("other", &["src/lib.rs"]);
    let shop_project = shop.as_project(None);
    let other_project = other.as_project(None);
    let (shop_name, other_name) = (shop_project.name.clone(), other_project.name.clone());
    let app = common::app(vec![shop_project, other_project], Recorded::listing(&[]))
        .with_forge(forge)
        .with_checkout(checkout);
    let mut harness = common::harness(app);

    // Opens "shop" and picks pull request "#41" from the list: the review
    // session starts checking it out and mapping its head, but this test
    // never waits for either to finish.
    common::double_click(&mut harness, &shop_name);
    common::await_read(&mut harness);
    harness.key_press(Key::P);
    harness.run_steps(5);
    await_labelled(&mut harness, "#41");
    harness.get_by_label("#41 · STORY-012").click_accesskit();
    harness.run_steps(5);

    // Esc, at the project layer, closes "shop" the ordinary way (the review
    // never became active, so `Navigation`'s own review state was never set,
    // and this is an ordinary `Moved::Closed`), back to the universe. Bounded
    // stepping throughout from here: the stale session may still be pulsing
    // (or about to start), and `common::double_click`/`common::await_read`
    // both call the unbounded `Harness::run()`, which never settles while
    // something pulses.
    harness.key_press(Key::Escape);
    harness.run_steps(5);

    // Opens the other project — an ordinary open, through the launcher, with
    // no pull request involved. A single click to select it, then Enter to
    // open it, rather than a double-click: two plain `.click()` calls this
    // late in the test (after the review's own interactions) land on a
    // `Selected` row that has already been clicked before and does not
    // reliably double-click-detect in the harness, unlike Enter on the
    // current selection, the same path `cellworks_navigation::Navigation::enter`
    // already exercises for every other "open from the launcher" test.
    harness.get_by_label(&other_name).click();
    harness.run_steps(5);
    harness.key_press(Key::Enter);
    harness.run_steps(5);

    // A moment for whatever the stale session's background checkout/mapping
    // was doing to finish, then lets the other project's own read finish too
    // (`common::await_read_bounded`, bounded stepping throughout since a
    // stale pull request could still be pulsing): neither may have any
    // further effect, however late it arrives, once a different project has
    // been opened the ordinary way.
    std::thread::sleep(Duration::from_millis(200));
    common::await_read_bounded(&mut harness, Duration::from_secs(10), 5);

    assert_eq!(
        breadcrumb(&harness),
        "Universe › other",
        "the project the engineer actually opened is shown, never the stale review"
    );
    let found = common::labels(&harness);
    assert!(
        !found.iter().any(|l| l.contains("STORY-012")),
        "the stale review's list/panel must not render over the new project: {found:?}"
    );
}
