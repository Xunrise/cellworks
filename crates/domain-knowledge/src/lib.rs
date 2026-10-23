//! The DomainKnowledge bounded context: parsing context cells and linking them
//! to code by glossary terms.
//!
//! Defined by docs/domain/contexts/DomainKnowledge/DomainKnowledge.md. [`read`]
//! reads the bounded contexts a project documents in `docs/domain/` (each
//! with its description, the first non-empty line after its `# Context:`
//! heading), the relationships of its context map, the Accepted decisions
//! indexed on `docs/arc42/09-decisions.md` with their scope and decision
//! line, and the story titles and "so that" known locally from
//! `sdlc/*/issue.md` — for the Map's cells lens and why panel (STORY-019).
//! [`read`] is an ordinary blocking function and [`start_read`] is the thread
//! around it.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use cellworks_map::{BoundedContext, BoundedContexts, Decision, Road, Scope, StoryText};

/// Something reading a project's domain knowledge has finished doing.
#[derive(Clone, Debug, PartialEq)]
pub enum DomainKnowledgeEvent {
    /// The project's bounded contexts were read: `None` when it documents
    /// none.
    ContextsRead {
        project: PathBuf,
        contexts: Option<BoundedContexts>,
    },
}

/// The bounded contexts the project at `root` documents, one per folder of
/// `docs/domain/contexts`, each with its description; the relationships of
/// the table in `docs/domain/context-map.md`; the project's Accepted
/// decisions; and the story text known locally (STORY-019). `code` maps a
/// context to the folders, relative to `root`, the project's settings say
/// implement it. `None` when the project documents no context.
///
/// Blocking: called on the thread [`start_read`] spawns, and from a test.
pub fn read(root: &Path, code: &BTreeMap<String, Vec<PathBuf>>) -> Option<BoundedContexts> {
    let mut names: Vec<String> = fs::read_dir(root.join("docs/domain/contexts"))
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    if names.is_empty() {
        return None;
    }
    names.sort();
    let contexts = names
        .into_iter()
        .map(|name| BoundedContext {
            code: code
                .get(&name)
                .map(|folders| folders.iter().map(|folder| root.join(folder)).collect())
                .unwrap_or_default(),
            description: context_description(root, &name),
            name,
        })
        .collect();
    let table = fs::read_to_string(root.join("docs/domain/context-map.md")).unwrap_or_default();
    Some(BoundedContexts {
        contexts,
        roads: roads(&table),
        decisions: read_decisions(root),
        stories: read_stories(root),
    })
}

/// The first non-empty line after the `# Context:` heading of
/// `docs/domain/contexts/<name>/<name>.md`; empty when the file or the
/// heading is missing (STORY-019).
fn context_description(root: &Path, name: &str) -> String {
    let path = root.join(format!("docs/domain/contexts/{name}/{name}.md"));
    let Ok(text) = fs::read_to_string(path) else {
        return String::new();
    };
    let mut lines = text.lines();
    if !lines
        .by_ref()
        .any(|line| line.trim_start().starts_with("# Context:"))
    {
        return String::new();
    }
    lines
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .unwrap_or_default()
        .to_owned()
}

/// The Accepted decisions indexed on `docs/arc42/09-decisions.md`, with the
/// decision line read from each ADR file; a missing index or a line that
/// fails to parse is skipped, never a failure (STORY-019).
fn read_decisions(root: &Path) -> Vec<Decision> {
    let index = fs::read_to_string(root.join("docs/arc42/09-decisions.md")).unwrap_or_default();
    index
        .lines()
        .filter_map(|line| decision_of_line(root, line))
        .collect()
}

/// One Accepted decision from an index line of `docs/arc42/09-decisions.md`:
/// `- [ADR-NNNN: Title](../adr/file.md) — Accepted — category · Scope, Names`.
/// `None` for a line that is not an Accepted decision, or that does not
/// parse (STORY-019).
fn decision_of_line(root: &Path, line: &str) -> Option<Decision> {
    let line = line.trim();
    let rest = line.strip_prefix("- [")?;
    let (link_text, rest) = rest.split_once(']')?;
    let rest = rest.strip_prefix('(')?;
    let (link_path, rest) = rest.split_once(')')?;
    let (id, title) = link_text.split_once(": ")?;

    let mut fields = rest.split('—').map(str::trim);
    fields.next()?; // the text between ")" and the first "—", always empty.
    if fields.next()? != "Accepted" {
        return None;
    }
    let tail = fields.next().unwrap_or_default();
    let scope = match tail.split_once('·') {
        Some((_, names)) if names.trim().eq_ignore_ascii_case("cross-cutting") => {
            Scope::CrossCutting
        }
        Some((_, names)) => Scope::Contexts(
            names
                .split(',')
                .map(|name| name.trim().to_owned())
                .collect(),
        ),
        None => Scope::CrossCutting,
    };

    let adr_path = root.join("docs/arc42").join(link_path);
    Some(Decision {
        id: id.to_owned(),
        title: title.to_owned(),
        line: adr_decision_line(&adr_path),
        scope,
    })
}

/// The first sentence of an ADR file's `## Decision` section; empty when the
/// file or the heading is missing (STORY-019).
fn adr_decision_line(path: &Path) -> String {
    let Ok(text) = fs::read_to_string(path) else {
        return String::new();
    };
    let mut lines = text.lines();
    if !lines.by_ref().any(|line| line.trim() == "## Decision") {
        return String::new();
    }
    let paragraph = lines
        .take_while(|line| !line.trim_start().starts_with("##"))
        .collect::<Vec<_>>()
        .join(" ");
    let paragraph: String = paragraph.split_whitespace().collect::<Vec<_>>().join(" ");
    match paragraph.split_once(". ") {
        Some((first, _)) => format!("{first}."),
        None => paragraph,
    }
}

/// The story title and "so that" of every `sdlc/*/issue.md`, keyed by the
/// story's normalised id; a missing `sdlc/` folder, or an `issue.md` that
/// does not match the expected shape, is skipped, never a failure
/// (STORY-019).
fn read_stories(root: &Path) -> BTreeMap<String, StoryText> {
    let Ok(entries) = fs::read_dir(root.join("sdlc")) else {
        return BTreeMap::new();
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| fs::read_to_string(entry.path().join("issue.md")).ok())
        .filter_map(|text| story_of_issue(&text))
        .collect()
}

/// A story's normalised id and text from the contents of an `issue.md`: the
/// id and title from its "# Issue #N: STORY-NNN: Title" heading, and "so
/// that" from the first "So that …" line of its Gherkin feature; `None` when
/// the heading does not match that shape. "So that" stays empty when no such
/// line exists (STORY-019).
fn story_of_issue(text: &str) -> Option<(String, StoryText)> {
    let heading = text
        .lines()
        .find(|line| line.trim_start().starts_with("# Issue"))?;
    let mut parts = heading.splitn(3, ": ");
    parts.next()?;
    let id = parts.next()?.trim();
    let title = parts.next()?.trim();
    Some((
        normalise_story_id(id),
        StoryText {
            title: title.to_owned(),
            so_that: so_that(text),
        },
    ))
}

/// A story id, upper-cased with its number padded to at least 3 digits, so
/// "story-12" becomes "STORY-012" (STORY-019, ADR-0008's convention).
fn normalise_story_id(id: &str) -> String {
    let id = id.trim().to_uppercase();
    match id.split_once('-') {
        Some((prefix, number)) => format!("{prefix}-{number:0>3}"),
        None => id,
    }
}

/// The text of the first "So that …" line in a Gherkin feature; empty when
/// there is none (STORY-019).
fn so_that(text: &str) -> String {
    text.lines()
        .find_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix("So that ")
                .or_else(|| trimmed.strip_prefix("so that "))
        })
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// The relationships of a context map's table, one row per relationship:
/// `| 1 | Map | CodeAnalysis | Published Language (PL) | … |`. Rows that are
/// not numbered relationships are left out.
fn roads(table: &str) -> Vec<Road> {
    table
        .lines()
        .filter_map(|line| {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            if cells.len() < 5 || cells[1].parse::<u32>().is_err() {
                return None;
            }
            let kind = cells[4]
                .split('(')
                .next()
                .unwrap_or_default()
                .trim()
                .to_lowercase();
            Some(Road {
                upstream: cells[2].to_owned(),
                downstream: cells[3].to_owned(),
                kind,
            })
        })
        .collect()
}

/// Runs [`read`] on a `std::thread` and hands back the receiving end.
pub fn start_read(
    root: PathBuf,
    code: BTreeMap<String, Vec<PathBuf>>,
) -> mpsc::Receiver<DomainKnowledgeEvent> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let contexts = read(&root, &code);
        // A send that fails is the UI having gone away.
        let _ = sender.send(DomainKnowledgeEvent::ContextsRead {
            project: root,
            contexts,
        });
    });
    receiver
}

#[cfg(test)]
mod tests {
    use cellworks_map::Scope;

    use super::*;

    fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
        let tree = tempfile::tempdir().expect("a temporary directory");
        for (file, text) in files {
            let path = tree.path().join(file);
            fs::create_dir_all(path.parent().expect("a parent")).expect("create the folder");
            fs::write(path, text).expect("write the file");
        }
        tree
    }

    #[test]
    fn every_context_folder_is_a_bounded_context_and_every_numbered_row_a_road() {
        let tree = project(&[
            ("docs/domain/contexts/Pricing/Pricing.md", "# Pricing\n"),
            ("docs/domain/contexts/Ordering/Ordering.md", "# Ordering\n"),
            (
                "docs/domain/context-map.md",
                "| # | Upstream | Downstream | Type |\n|---|---|---|---|\n\
                 | 1 | Pricing | Ordering | Published Language (PL) | confirmed |\n",
            ),
        ]);
        let code = BTreeMap::from([("Pricing".to_owned(), vec![PathBuf::from("crates/catalog")])]);

        let known = read(tree.path(), &code).expect("contexts");

        let names: Vec<&str> = known.contexts.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Ordering", "Pricing"]);
        assert_eq!(known.contexts[1].code, [tree.path().join("crates/catalog")]);
        assert!(known.contexts[0].code.is_empty());
        assert_eq!(
            known.roads,
            [Road {
                upstream: "Pricing".to_owned(),
                downstream: "Ordering".to_owned(),
                kind: "published language".to_owned(),
            }]
        );
    }

    #[test]
    fn a_project_without_a_domain_model_documents_no_context() {
        let tree = project(&[("README.md", "# notes\n")]);

        assert_eq!(read(tree.path(), &BTreeMap::new()), None);
    }

    #[test]
    fn a_context_has_the_first_line_after_its_heading_as_description() {
        let tree = project(&[(
            "docs/domain/contexts/Pricing/Pricing.md",
            "# Context: Pricing\n\n\
             How much a customer pays for an order.\n\n\
             More detail follows below the first line.\n",
        )]);

        let known = read(tree.path(), &BTreeMap::new()).expect("contexts");

        assert_eq!(
            known.contexts[0].description,
            "How much a customer pays for an order."
        );
    }

    #[test]
    fn adr_scopes_are_read_from_the_decisions_index() {
        let tree = project(&[
            (
                "docs/domain/contexts/Pricing/Pricing.md",
                "# Context: Pricing\n\nHow much a customer pays for an order.\n",
            ),
            (
                "docs/arc42/09-decisions.md",
                "# 9. Architecture Decisions\n\n\
                 - [ADR-0001: Use eframe/egui as the UI framework](../adr/0001-egui.md) — Accepted — \
                 technology · Pricing, Ordering\n",
            ),
            (
                "docs/adr/0001-egui.md",
                "# ADR-0001: Use eframe/egui as the UI framework\n\n\
                 ## Decision\n\
                 We will build the UI with eframe/egui. It draws the map on a custom-painted canvas.\n",
            ),
        ]);

        let known = read(tree.path(), &BTreeMap::new()).expect("contexts");

        assert_eq!(known.decisions.len(), 1);
        let decision = &known.decisions[0];
        assert_eq!(decision.id, "ADR-0001");
        assert_eq!(decision.title, "Use eframe/egui as the UI framework");
        assert_eq!(decision.line, "We will build the UI with eframe/egui.");
        assert_eq!(
            decision.scope,
            Scope::Contexts(vec!["Pricing".to_owned(), "Ordering".to_owned()])
        );
    }

    #[test]
    fn an_adr_without_scope_is_cross_cutting() {
        let tree = project(&[
            (
                "docs/domain/contexts/Pricing/Pricing.md",
                "# Context: Pricing\n\nHow much a customer pays for an order.\n",
            ),
            (
                "docs/arc42/09-decisions.md",
                "# 9. Architecture Decisions\n\n\
                 - [ADR-0002: One crate per bounded context](../adr/0002-workspace.md) — Accepted — \
                 structure · cross-cutting\n\
                 - [ADR-0003: Integrate external tools as child processes](../adr/0003-tools.md) — Accepted — \
                 technology\n",
            ),
            (
                "docs/adr/0002-workspace.md",
                "# ADR-0002: One crate per bounded context\n\n\
                 ## Decision\n\
                 Each bounded context gets its own crate in the workspace.\n",
            ),
            (
                "docs/adr/0003-tools.md",
                "# ADR-0003: Integrate external tools as child processes\n\n\
                 ## Decision\n\
                 We shell out to git, kubectl, nvim and curl as child processes.\n",
            ),
        ]);

        let known = read(tree.path(), &BTreeMap::new()).expect("contexts");

        assert_eq!(known.decisions.len(), 2);
        // Explicitly scoped "cross-cutting" …
        assert_eq!(known.decisions[0].scope, Scope::CrossCutting);
        // … and a missing "· <scope>" suffix altogether: both mean the same.
        assert_eq!(known.decisions[1].scope, Scope::CrossCutting);
    }

    #[test]
    fn only_accepted_adrs_are_read() {
        let tree = project(&[
            (
                "docs/domain/contexts/Pricing/Pricing.md",
                "# Context: Pricing\n\nHow much a customer pays for an order.\n",
            ),
            (
                "docs/arc42/09-decisions.md",
                "# 9. Architecture Decisions\n\n\
                 - [ADR-0001: Settled](../adr/0001-settled.md) — Accepted — technology · Pricing\n\
                 - [ADR-0002: Still open](../adr/0002-open.md) — Proposed — technology · Pricing\n",
            ),
            (
                "docs/adr/0001-settled.md",
                "# ADR-0001: Settled\n\n## Decision\nWe keep this decision.\n",
            ),
            (
                "docs/adr/0002-open.md",
                "# ADR-0002: Still open\n\n## Decision\nWe might keep this decision.\n",
            ),
        ]);

        let known = read(tree.path(), &BTreeMap::new()).expect("contexts");

        let ids: Vec<&str> = known.decisions.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, ["ADR-0001"]);
    }

    #[test]
    fn stories_are_read_from_issue_files_by_normalised_id() {
        let tree = project(&[
            (
                "docs/domain/contexts/Pricing/Pricing.md",
                "# Context: Pricing\n\nHow much a customer pays for an order.\n",
            ),
            (
                "sdlc/0007-story-12-loyalty-discount/issue.md",
                "# Issue #7: STORY-12: Loyalty discount\n\
                 Reported by: someone\n\n\
                 Feature: Loyalty discount\n\
                 \x20\x20As a customer\n\
                 \x20\x20I want a discount for being loyal\n\
                 \x20\x20So that regular customers feel rewarded\n",
            ),
        ]);

        let known = read(tree.path(), &BTreeMap::new()).expect("contexts");

        let story = known.stories.get("STORY-012").expect("story STORY-012");
        assert_eq!(story.title, "Loyalty discount");
        assert_eq!(story.so_that, "regular customers feel rewarded");
    }

    #[test]
    fn a_project_without_adrs_or_stories_reads_empty_lists() {
        let tree = project(&[(
            "docs/domain/contexts/Pricing/Pricing.md",
            "# Context: Pricing\n\nHow much a customer pays for an order.\n",
        )]);

        let known = read(tree.path(), &BTreeMap::new()).expect("contexts");

        assert!(known.decisions.is_empty());
        assert!(known.stories.is_empty());
    }
}
