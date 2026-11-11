//! What DomainKnowledge tells the Map for the cells lens and the why panel:
//! the bounded contexts a project documents, the code that implements each,
//! the context map's relationships between them, the decisions that bind a
//! context and the stories known locally.

use std::path::{Path, PathBuf};

/// A project's bounded contexts and the roads between them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoundedContexts {
    pub contexts: Vec<BoundedContext>,
    pub roads: Vec<Road>,
    /// Accepted ADRs, read from `docs/arc42/09-decisions.md` and the ADR
    /// files it indexes; empty where none were read (STORY-019).
    pub decisions: Vec<Decision>,
    /// Story title and "so that", keyed by the normalised story id (upper
    /// case, at least three digits); empty where none were read (STORY-019).
    pub stories: std::collections::BTreeMap<String, StoryText>,
}

/// One bounded context.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundedContext {
    pub name: String,
    /// The folders that implement it, canonical, as the project's settings
    /// name them; empty where they do not, and then the folder of the same
    /// name does.
    pub code: Vec<PathBuf>,
    /// The first non-empty line after the `# Context:` heading of its
    /// `docs/domain/contexts/<Name>/<Name>.md`; empty where unread
    /// (STORY-019).
    pub description: String,
}

/// A relationship of the context map: upstream, downstream, and its type
/// as the engineer reads it ("published language", "conformist", …).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Road {
    pub upstream: String,
    pub downstream: String,
    pub kind: String,
}

/// An Accepted ADR, indexed on `docs/arc42/09-decisions.md`: its id, title,
/// the first sentence of its `## Decision`, and the bounded contexts its
/// scope names (STORY-019).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
    pub id: String,
    pub title: String,
    pub line: String,
    pub scope: Scope,
}

impl Decision {
    /// Whether this decision binds a tower in `context`: a cross-cutting
    /// decision always does; one scoped to contexts binds only a tower
    /// whose context is named among them, never a tower in no context.
    pub fn binds(&self, context: Option<&str>) -> bool {
        match &self.scope {
            Scope::CrossCutting => true,
            Scope::Contexts(names) => context.is_some_and(|name| names.iter().any(|n| n == name)),
        }
    }
}

/// The reach of a decision: every context (the Jarvis `adr-writing` index
/// line's `· <scope>` suffix), or none, meaning cross-cutting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    CrossCutting,
    Contexts(Vec<String>),
}

/// A story's title and "so that", read locally from its `sdlc/*/issue.md`
/// (STORY-019).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StoryText {
    pub title: String,
    pub so_that: String,
}

/// A name as a key: letters and digits only, in lower case, so the folder
/// `code-analysis` implements the context `CodeAnalysis`.
fn key(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

impl BoundedContexts {
    /// The bounded context `file` belongs to: the deepest folder any
    /// context's settings name as an ancestor of `file` wins; failing that,
    /// the nearest ancestor directory of `file` whose name key-folds to
    /// match a context with no settings folders of its own; otherwise none
    /// (STORY-019, moved here from the cells lens's `deeper` and
    /// `context_of`).
    pub fn context_of_file(&self, file: &Path) -> Option<&BoundedContext> {
        let named = self
            .contexts
            .iter()
            .flat_map(|context| context.code.iter().map(move |folder| (context, folder)))
            .filter(|(_, folder)| file.starts_with(folder))
            .max_by_key(|(_, folder)| folder.as_os_str().len())
            .map(|(context, _)| context);
        named.or_else(|| {
            file.ancestors().skip(1).find_map(|folder| {
                let name = folder.file_name()?.to_str()?;
                self.contexts
                    .iter()
                    .find(|c| c.code.is_empty() && key(&c.name) == key(name))
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// A name as a key: letters and digits only, in lower case, so the
    /// folder `code-analysis` implements the context `CodeAnalysis`; the
    /// rule `BoundedContexts::context_of_file` uses, moved here from the
    /// cells lens (STORY-019).
    #[test]
    fn a_folder_implements_the_context_named_like_it() {
        assert_eq!(key("code-analysis"), key("CodeAnalysis"));
        assert_eq!(key("domain_knowledge"), key("DomainKnowledge"));
        assert_ne!(key("map"), key("Navigation"));
    }

    #[test]
    fn a_tower_in_no_context_has_none() {
        let known = BoundedContexts {
            contexts: vec![
                BoundedContext {
                    name: "Map".to_owned(),
                    code: vec![PathBuf::from("crates/map")],
                    description: String::new(),
                },
                BoundedContext {
                    name: "DomainKnowledge".to_owned(),
                    code: Vec::new(),
                    description: String::new(),
                },
            ],
            ..Default::default()
        };

        // A folder named in a context's settings wins, however deep the file.
        assert_eq!(
            known
                .context_of_file(Path::new("crates/map/src/contexts.rs"))
                .map(|c| c.name.as_str()),
            Some("Map")
        );

        // No settings name an ancestor, but a nearer ancestor folder's name
        // matches a context with no settings of its own.
        assert_eq!(
            known
                .context_of_file(Path::new("crates/domain-knowledge/src/lib.rs"))
                .map(|c| c.name.as_str()),
            Some("DomainKnowledge")
        );

        // Neither a settings ancestor nor a matching folder name: no context.
        assert_eq!(known.context_of_file(Path::new("docs/readme.md")), None);
    }

    #[test]
    fn decisions_follow_the_context_or_are_cross_cutting() {
        let decisions = [
            Decision {
                id: "ADR-0001".to_owned(),
                title: "Use eframe/egui".to_owned(),
                line: "We will build the UI with eframe/egui.".to_owned(),
                scope: Scope::CrossCutting,
            },
            Decision {
                id: "ADR-0002".to_owned(),
                title: "Lens layouts by community detection".to_owned(),
                line: "We lay out a lens by community detection.".to_owned(),
                scope: Scope::Contexts(vec!["Map".to_owned()]),
            },
            Decision {
                id: "ADR-0003".to_owned(),
                title: "Custom arrangement store".to_owned(),
                line: "We keep the custom arrangement in Cellworks' own data directory.".to_owned(),
                scope: Scope::Contexts(vec!["Workspace".to_owned()]),
            },
        ];

        // A tower in the "Map" context sees its own decision and the
        // cross-cutting one, not the one scoped to another context.
        let for_map: Vec<&str> = decisions
            .iter()
            .filter(|d| d.binds(Some("Map")))
            .map(|d| d.id.as_str())
            .collect();
        assert_eq!(for_map, ["ADR-0001", "ADR-0002"]);

        // A tower in no context sees only the cross-cutting decisions.
        let for_none: Vec<&str> = decisions
            .iter()
            .filter(|d| d.binds(None))
            .map(|d| d.id.as_str())
            .collect();
        assert_eq!(for_none, ["ADR-0001"]);
    }
}
