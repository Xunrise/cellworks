//! STORY-030, sessions walk to the towers they work on.
//!
//! One test per scenario of issue #52, named after it. The Background has
//! the project "shop" open as a city and the session "pricing-fix" running
//! in it; every session here is seeded by hand through
//! `CellworksApp::with_sessions`, never through a real transcript or
//! `claude`. "shop" is the folder tree `common::shop_city` (or, where a
//! scenario needs "shop" to be a git checkout, `common::shop_tree`)
//! describes: "catalog/pricing.rs" holds the function "discount" on its
//! first 12 lines. No test here runs git, `claude`, reads `$HOME` or opens a
//! window.

mod common;

use std::path::PathBuf;
use std::sync::Arc;

use cellworks::app::CellworksApp;
use cellworks_agent_sessions::{FileEvent, Session, SessionState, groups, place};
use cellworks_code_analysis::GitListing;
use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use common::{Recorded, Tree, shop_city, shop_common_dir, shop_tree, shop_worktree};

/// Opens `tree`'s project (as "shop") seeded with `sessions`, the way
/// `common::open` opens a project seeded with a git listing.
fn open_with_sessions(tree: &Tree, sessions: Vec<Session>) -> Harness<'static, CellworksApp> {
    let project = tree.as_project(None);
    let name = project.name.clone();
    // A `.git` folder (`shop_tree`, unlike `shop_city`) makes `load` trust
    // git over walking the folder, so the recorded listing must name the
    // files `shop_tree_sources` writes, or the map comes up empty.
    let git: Arc<dyn GitListing + Send + Sync> = if tree.root.join(".git").exists() {
        Recorded::listing(&["catalog/pricing.rs", "catalog/mod.rs", "orders/order.rs"])
    } else {
        Recorded::listing(&[])
    };
    let app = common::app(vec![project], git).with_sessions(sessions);
    let mut harness = common::harness(app);
    common::double_click(&mut harness, &name);
    common::await_read(&mut harness);
    harness
}

/// A session named `id`, with `status`/`state` and the folders "shop" and
/// "main" as its repository and branch, touching `events`.
fn session(id: &str, status: &str, state: &str, events: Vec<FileEvent>) -> Session {
    session_in(id, status, state, "shop", "main", events)
}

/// [`session`] with an explicit repo and branch, for a session that belongs
/// to a different project.
fn session_in(
    id: &str,
    status: &str,
    state: &str,
    repo: &str,
    branch: &str,
    events: Vec<FileEvent>,
) -> Session {
    Session {
        id: id.to_owned(),
        name: id.to_owned(),
        cwd: PathBuf::new(),
        status: status.to_owned(),
        state: state.to_owned(),
        repo: repo.to_owned(),
        branch: branch.to_owned(),
        summary: String::new(),
        last_active: Some(0),
        attach_id: None,
        events,
    }
}

/// A read of `path`, with no checkout (an absolute path, matched to a
/// project's city by a path prefix - the fixtures here are not git
/// checkouts unless the scenario says so).
fn read_of(path: PathBuf, line: Option<usize>) -> FileEvent {
    FileEvent {
        write: false,
        checkout: None,
        path,
        line,
        added: 0,
    }
}

/// A session walks to the tower it reads
#[test]
fn a_session_walks_to_the_tower_it_reads() {
    let (tree, _opened_without_sessions) = shop_city();
    let discount = tree.root.join("catalog/pricing.rs").join("discount");
    let pricing_fix = session(
        "pricing-fix",
        "busy",
        "working",
        vec![read_of(tree.root.join("catalog/pricing.rs"), Some(3))],
    );

    let harness = open_with_sessions(&tree, vec![pricing_fix]);

    let drawn = harness.state().sessions_drawn();
    assert_eq!(drawn.len(), 1, "{drawn:?}");
    assert_eq!(drawn[0].id, "pricing-fix");
    assert_eq!(
        drawn[0].tower, discount,
        "seen for the first time, it already stands at its tower"
    );
    assert!(!drawn[0].waiting);
    assert!(!drawn[0].writing);
}

/// Naming a file in a command counts as reading it
#[test]
fn naming_a_file_in_a_command_counts_as_reading_it() {
    let (tree, _opened_without_sessions) = shop_city();
    let pricing_rs = tree.root.join("catalog/pricing.rs");
    // A Bash, Grep or Glob call names a file with no line; the end-to-end
    // wiring from such an event to a drawn tower is what this scenario
    // pins (the Bash/Grep/Glob tokenising rules themselves are pinned by
    // `transcript::tests::tool_calls_become_file_events`).
    let pricing_fix = session(
        "pricing-fix",
        "busy",
        "working",
        vec![read_of(pricing_rs, None)],
    );

    let harness = open_with_sessions(&tree, vec![pricing_fix.clone()]);

    let map = common::map(&harness);
    let expected = place(&map, None, &pricing_fix)
        .expect("naming an existing file of this city still places the session");
    let drawn = harness.state().sessions_drawn();
    assert_eq!(drawn.len(), 1, "{drawn:?}");
    assert_eq!(
        drawn[0].tower, expected.tower,
        "a tower of pricing.rs - its node, or its first function"
    );
}

/// Writing adds floors while the session writes
#[test]
fn writing_adds_floors_while_the_session_writes() {
    let (tree, _opened_without_sessions) = shop_city();
    let discount = tree.root.join("catalog/pricing.rs").join("discount");
    let writing_event = FileEvent {
        write: true,
        checkout: None,
        path: tree.root.join("catalog/pricing.rs"),
        line: Some(3),
        added: 3,
    };
    let pricing_fix = session("pricing-fix", "busy", "working", vec![writing_event]);

    let harness = open_with_sessions(&tree, vec![pricing_fix.clone()]);

    // `DrawnSession` does not carry how many floors are being written, so
    // the growth itself is asserted through `place`, the same placement the
    // overlay paints from.
    let map = common::map(&harness);
    let placed = place(&map, None, &pricing_fix).expect("the write lands on a tower");
    assert_eq!(placed.tower, discount);
    assert!(placed.writing, "the floors stand marked as being written");
    assert_eq!(
        placed.added, 3,
        "the tower grows by the floors being written"
    );

    let drawn = harness.state().sessions_drawn();
    assert_eq!(drawn.len(), 1, "{drawn:?}");
    assert_eq!(drawn[0].tower, discount);
    assert!(drawn[0].writing, "the drawn session shows it is writing");
}

/// A session waiting for the engineer asks for attention
#[test]
fn a_session_waiting_for_the_engineer_asks_for_attention() {
    let (tree, _opened_without_sessions) = shop_city();
    let discount = tree.root.join("catalog/pricing.rs").join("discount");
    let event = read_of(tree.root.join("catalog/pricing.rs"), Some(3));
    let pricing_fix = session("pricing-fix", "busy", "waiting", vec![event]);
    assert_eq!(
        pricing_fix.session_state(),
        SessionState::Waiting,
        "a waiting state asks for attention regardless of status"
    );

    let harness = open_with_sessions(&tree, vec![pricing_fix]);

    let drawn = harness.state().sessions_drawn();
    assert_eq!(drawn.len(), 1, "{drawn:?}");
    assert!(
        drawn[0].waiting,
        "it signals that it waits for the engineer"
    );
    assert_eq!(
        drawn[0].tower, discount,
        "it stays at the last tower it touched"
    );

    // The exact row text the sidebar renders is the implementer's choice;
    // what must hold is that `groups` - the data the sidebar draws from -
    // lists pricing-fix among the sessions that need the engineer. "Needs
    // you" is the group name design.md's requirement 10 pins.
    let map = common::map(&harness);
    let listed = groups(harness.state().sessions(), &map, None, "");
    let needs_you = listed
        .iter()
        .find(|(label, _)| *label == "Needs you")
        .map(|(_, members)| members);
    assert!(
        needs_you.is_some_and(|members| members.iter().any(|s| s.id == "pricing-fix")),
        "pricing-fix is listed among the sessions that need the engineer: {listed:?}"
    );
}

/// Sessions at the same tower stand apart
#[test]
fn sessions_at_the_same_tower_stand_apart() {
    let (tree, _opened_without_sessions) = shop_city();
    let discount = tree.root.join("catalog/pricing.rs").join("discount");
    let event = read_of(tree.root.join("catalog/pricing.rs"), Some(3));
    let pricing_fix = session("pricing-fix", "busy", "working", vec![event.clone()]);
    let docs_run = session("docs-run", "busy", "working", vec![event]);

    let harness = open_with_sessions(&tree, vec![pricing_fix, docs_run]);

    let drawn = harness.state().sessions_drawn();
    let at_discount: Vec<_> = drawn.iter().filter(|s| s.tower == discount).collect();
    assert_eq!(at_discount.len(), 2, "{drawn:?}");
    assert_ne!(
        at_discount[0].dot, at_discount[1].dot,
        "both are shown at discount without covering each other"
    );
}

/// A session working in another checkout of the project is in the city
#[test]
fn a_session_working_in_another_checkout_of_the_project_is_in_the_city() {
    let (tree, _opened_without_sessions) = shop_tree();
    let worktree_root = shop_worktree(&tree, "loyalty-proto-wt", "loyalty-proto");
    // Its files are never read by Cellworks, only named by the session's
    // events below; the worktree folder only needs to exist on disk.
    assert!(worktree_root.is_dir());
    let common_dir = shop_common_dir(&tree);
    let discount = tree.root.join("catalog/pricing.rs").join("discount");
    let loyalty_proto = session(
        "loyalty-proto",
        "busy",
        "working",
        vec![FileEvent {
            write: false,
            checkout: Some(common_dir),
            path: PathBuf::from("catalog/pricing.rs"),
            line: Some(3),
            added: 0,
        }],
    );

    let harness = open_with_sessions(&tree, vec![loyalty_proto]);

    let drawn = harness.state().sessions_drawn();
    assert_eq!(drawn.len(), 1, "{drawn:?}");
    assert_eq!(
        drawn[0].tower, discount,
        "the city of the open 'shop', not a separate one"
    );
}

/// A session in another project is not in this city
#[test]
fn a_session_in_another_project_is_not_in_this_city() {
    let (tree, _opened_without_sessions) = shop_city();
    let handbook = Tree::project("handbook", &["intro.md"]);
    let docs_run = session_in(
        "docs-run",
        "idle",
        "done",
        "handbook",
        "main",
        vec![read_of(handbook.root.join("intro.md"), None)],
    );

    let harness = open_with_sessions(&tree, vec![docs_run]);

    assert!(
        harness.state().sessions_drawn().is_empty(),
        "docs-run touches no file of shop's city"
    );
    let map = common::map(&harness);
    let listed = groups(harness.state().sessions(), &map, None, "");
    let other_projects = listed
        .iter()
        .find(|(label, _)| *label == "Other projects")
        .map(|(_, members)| members);
    assert!(
        other_projects.is_some_and(|members| members.iter().any(|s| s.id == "docs-run")),
        "the sidebar lists docs-run under other projects: {listed:?}"
    );
}

/// Selecting a session in the sidebar brings it into view
#[test]
fn selecting_a_session_in_the_sidebar_brings_it_into_view() {
    let (tree, _opened_without_sessions) = shop_city();
    let discount = tree.root.join("catalog/pricing.rs").join("discount");
    let event = read_of(tree.root.join("catalog/pricing.rs"), Some(3));
    let pricing_fix = session("pricing-fix", "busy", "working", vec![event]);

    let mut harness = open_with_sessions(&tree, vec![pricing_fix]);
    // The Explorer, open beside the map by default (STORY-089), drives the
    // arrows itself while open; hidden, they pan the camera as this test
    // means them to.
    harness.key_press_modifiers(Modifiers::COMMAND, Key::B);
    harness.run();
    // Pan the camera away first, so "discount" does not already start in
    // the middle of the view.
    for key in [Key::ArrowRight, Key::ArrowDown] {
        for _ in 0..5 {
            harness.key_press(key);
            harness.run();
        }
    }
    let map = common::map(&harness);
    let discount_centre = map
        .item(&discount)
        .expect("discount is on the map")
        .area
        .center();
    let before = harness.state().view().expect("a view").visible;
    assert!(
        (before.center() - discount_centre).length() > 5.0,
        "discount starts away from the centre of the view: {before:?} vs {discount_centre:?}"
    );

    // The exact accesskit label an implementer gives a sidebar row is its
    // own choice; the session's own name is the natural one.
    harness.get_by_label("pricing-fix").click_accesskit();
    harness.run();

    let after = harness.state().view().expect("a view").visible;
    assert!(
        (after.center() - discount_centre).length() < 5.0,
        "the city centred on pricing-fix's tower: {after:?} vs {discount_centre:?}"
    );
}

/// A session that ended leaves the city
#[test]
fn a_session_that_ended_leaves_the_city() {
    let (tree, _opened_without_sessions) = shop_city();
    let event = read_of(tree.root.join("catalog/pricing.rs"), Some(3));
    let pricing_fix = session("pricing-fix", "busy", "working", vec![event]);

    let running = open_with_sessions(&tree, vec![pricing_fix]);
    assert_eq!(
        running.state().sessions().len(),
        1,
        "it is running at first"
    );
    assert_eq!(running.state().sessions_drawn().len(), 1);

    // "pricing-fix" ends: `claude agents --json` no longer lists it, so a
    // watch reports none. There is no API to end a session that is already
    // running inside a test, so the rule is modelled structurally: a
    // session `claude agents --json` stops naming simply never appears -
    // the same tree, opened with no sessions at all, shows and lists none.
    let ended = open_with_sessions(&tree, Vec::new());

    assert!(
        ended.state().sessions().is_empty(),
        "it is no longer listed anywhere"
    );
    assert!(
        ended.state().sessions_drawn().is_empty(),
        "it is no longer shown in the city"
    );
}
