//! The why panel: what the selected tower is, who built it and what binds
//! it, written in the right column beside the city (STORY-019).
//!
//! All of it is plain labels, like `ui/explain.rs`, so the accessibility
//! tree reads every line; the colours are the palette's own.

use egui::{Color32, RichText};

use super::{DIM, INK, PR_HUES};
use crate::history::Commit;
use crate::why::{PrKey, Why};

/// The headings of the panel, so a test and a reader name them the same way.
const CONTEXT: &str = "Context";
const BUILT_BY: &str = "Built by";
const DECISIONS: &str = "Decisions";

/// What a tower in no documented bounded context says.
pub const NO_CONTEXT: &str = "No bounded context";

/// What a tower says whose floors all come from commits naming no story.
pub const NO_STORY: &str = "Built by commits without a story";

/// What the decisions section says when none binds the tower.
const NO_DECISION: &str = "No decision binds this tower.";

/// Writes why the tower of `why` was built into `ui`: its bounded context
/// and that context's description, the pull requests that built its floors
/// with their shares and stories, and the decisions that bind it. With a
/// floor selected, `floor` is the commit that last wrote it, and the panel
/// adds that commit's subject and its stories' "so that".
pub fn why_panel(ui: &mut egui::Ui, why: &Why, floor: Option<&Commit>) {
    let name = why
        .item
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    ui.label(
        RichText::new(format!("Why {name} was built"))
            .strong()
            .color(INK),
    );
    ui.add_space(6.0);

    heading(ui, CONTEXT);
    match &why.context {
        Some((context, description)) => {
            ui.label(RichText::new(context).color(INK));
            if !description.is_empty() {
                ui.label(RichText::new(description).color(DIM));
            }
        }
        None => {
            ui.label(RichText::new(NO_CONTEXT).color(DIM));
        }
    }
    ui.add_space(10.0);

    heading(ui, BUILT_BY);
    if why.without_story() {
        ui.label(RichText::new(NO_STORY).color(INK));
    }
    for (index, share) in why.built_by.iter().enumerate() {
        let colour = match share.key {
            PrKey::Number(_) => PR_HUES[index % PR_HUES.len()],
            _ => DIM,
        };
        ui.label(
            RichText::new(format!(
                "{} · {} of {} floors ({} %)",
                share.key.label(),
                share.floors,
                why.floors.len(),
                percent(share.share),
            ))
            .color(colour),
        );
        for story in &share.stories {
            ui.label(RichText::new(format!("  {}", story.label())).color(INK));
            if let Some(text) = &story.text
                && !text.so_that.is_empty()
            {
                ui.label(RichText::new(format!("  So that {}", text.so_that)).color(DIM));
            }
        }
        // The commits themselves are what is left to read where no story
        // names them: a pull request with stories is told by its stories.
        if share.stories.is_empty() {
            for subject in &share.commits {
                ui.label(RichText::new(format!("  {subject}")).color(DIM));
            }
        }
    }
    ui.add_space(10.0);

    heading(ui, DECISIONS);
    if why.decisions.is_empty() {
        ui.label(RichText::new(NO_DECISION).color(DIM));
    }
    for decision in &why.decisions {
        ui.label(RichText::new(format!("{}: {}", decision.id, decision.title)).color(INK));
        if !decision.line.is_empty() {
            ui.label(RichText::new(&decision.line).color(DIM));
        }
    }

    // The floor the engineer stands on, inside the tower.
    let Some(commit) = floor else {
        return;
    };
    ui.add_space(10.0);
    heading(ui, "This floor");
    ui.label(RichText::new(&commit.summary).color(INK));
    for story in why.stories_of(commit) {
        ui.label(RichText::new(format!("  {}", story.label())).color(INK));
        if let Some(text) = &story.text
            && !text.so_that.is_empty()
        {
            ui.label(RichText::new(format!("  So that {}", text.so_that)).color(DIM));
        }
    }
}

/// The label a floor row wears inside a tower: `#<pr> STORY-NNN`, with
/// ` +k` for the further stories its commit names; `None` for an
/// uncommitted floor or a commit naming no pull request.
pub(super) fn floor_mark(commit: &Commit) -> Option<String> {
    let number = commit.pull_request?;
    let Some((first, rest)) = commit.stories.split_first() else {
        return Some(format!("#{number}"));
    };
    match rest.len() {
        0 => Some(format!("#{number} {first}")),
        more => Some(format!("#{number} {first} +{more}")),
    }
}

/// A share, 0 to 1, as whole percent.
fn percent(share: f32) -> u32 {
    (share * 100.0).round() as u32
}

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).strong().color(hue(INK)));
}

/// Headings are written in the ink of the panel, a shade back.
fn hue(colour: Color32) -> Color32 {
    colour.gamma_multiply(0.8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(pull_request: Option<u32>, stories: &[&str]) -> Commit {
        Commit {
            sha: "c1".to_owned(),
            time: 0,
            summary: "feat: a thing".to_owned(),
            pull_request,
            stories: stories.iter().map(|id| (*id).to_owned()).collect(),
        }
    }

    #[test]
    fn a_floor_is_marked_with_its_pull_request_and_stories() {
        assert_eq!(
            floor_mark(&commit(Some(12), &["STORY-012"])).as_deref(),
            Some("#12 STORY-012")
        );
        assert_eq!(
            floor_mark(&commit(Some(80), &["STORY-015", "STORY-016"])).as_deref(),
            Some("#80 STORY-015 +1"),
            "the further stories are counted, not listed"
        );
        assert_eq!(
            floor_mark(&commit(Some(9), &[])).as_deref(),
            Some("#9"),
            "a pull request naming no story"
        );
        assert_eq!(
            floor_mark(&commit(None, &["STORY-012"])),
            None,
            "a commit without a pull request is not marked"
        );
    }

    #[test]
    fn a_share_is_written_as_whole_percent() {
        assert_eq!(percent(7.0 / 9.0), 78);
        assert_eq!(percent(2.0 / 9.0), 22);
        assert_eq!(percent(1.0), 100);
    }
}
