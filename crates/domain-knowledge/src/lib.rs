//! The DomainKnowledge bounded context: parsing context cells and linking them
//! to code by glossary terms.
//!
//! Defined by docs/domain/contexts/DomainKnowledge/DomainKnowledge.md. So far
//! it reads the bounded contexts a project documents in `docs/domain/` and
//! the relationships of its context map, for the Map's cells lens. [`read`]
//! is an ordinary blocking function and [`start_read`] is the thread around
//! it.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use cellworks_map::{BoundedContext, BoundedContexts, Road};

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
/// `docs/domain/contexts`, and the relationships of the table in
/// `docs/domain/context-map.md`. `code` maps a context to the folders,
/// relative to `root`, the project's settings say implement it. `None` when
/// the project documents no context.
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
            name,
        })
        .collect();
    let table = fs::read_to_string(root.join("docs/domain/context-map.md")).unwrap_or_default();
    Some(BoundedContexts {
        contexts,
        roads: roads(&table),
    })
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
}
