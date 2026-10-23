//! Wiring for the Review mode's pull-request source: the binary turns a
//! checked-out worktree into a `cellworks_map::ProjectMap` (Sources may not
//! depend on CodeAnalysis, docs/arc42/05-building-blocks.md) and starts
//! listing and opening pull requests with the Sources adapters.
//!
//! No UI and no key binding in this story; STORY-022 consumes the events.

use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};

use cellworks_sources::pull_requests::{
    GhCli, GitCli, PullRequest, PullRequestEvent, start_list, start_open,
};

/// Maps a checkout at `root` the way the project is normally mapped, so a
/// pull request's lines land on the city at its head (requirement 6). `None`
/// when the read found nothing to map.
pub fn map_checkout(root: &Path) -> Option<cellworks_map::ProjectMap> {
    let events = cellworks_code_analysis::load(
        root,
        "review",
        cellworks_map::default_prefix_rule(),
        &cellworks_code_analysis::GitCli,
        false,
    );
    events.into_iter().find_map(|event| match event {
        cellworks_code_analysis::CodeAnalysisEvent::ProjectMapped { map, .. } => Some(*map),
        _ => None,
    })
}

/// Starts listing `repo`'s pull requests, with `gh` as the forge.
pub fn start_list_reviews(repo: PathBuf) -> mpsc::Receiver<PullRequestEvent> {
    start_list(repo, Arc::new(GhCli))
}

/// Starts opening `pull_request` of `repo`, with git as the checkout and
/// [`map_checkout`] mapping the temporary worktree.
pub fn start_open_review(
    repo: PathBuf,
    pull_request: PullRequest,
) -> mpsc::Receiver<PullRequestEvent> {
    start_open(repo, pull_request, Arc::new(GitCli), Arc::new(map_checkout))
}
