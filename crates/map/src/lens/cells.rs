//! The cells lens: every bounded context a district holding the files of
//! the code that implements it, code of no context a district of its own,
//! contexts the context map relates pulled together, and the relationships
//! kept as roads between them.

use std::collections::BTreeMap;
use std::path::Path;

use egui::Rect;

use super::cluster::{Weights, link};
use super::shape;
use crate::contexts::BoundedContexts;
use crate::graph::{self, District, DistrictKind, Node, ProjectMap};
use crate::layout;

/// A name as a key: letters and digits only, in lower case, so the folder
/// `code-analysis` implements the context `CodeAnalysis`.
fn key(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// The districts of `folders` by the bounded contexts of `known`, laid out,
/// and the roads between them as (upstream, downstream, type) by district.
pub(crate) fn cells(
    folders: &ProjectMap,
    known: &BoundedContexts,
) -> (Vec<District>, Vec<(usize, usize, String)>) {
    // Which context a folder district belongs to: named in the settings, or
    // else the context named like it.
    let context_of = |district: &District| -> Option<usize> {
        let named = known
            .contexts
            .iter()
            .position(|c| c.code.iter().any(|folder| folder == &district.path));
        named.or_else(|| {
            known
                .contexts
                .iter()
                .position(|c| c.code.is_empty() && key(&c.name) == key(&district.name))
        })
    };
    let mut held: BTreeMap<usize, Vec<Node>> = BTreeMap::new();
    let mut outside: Vec<Node> = Vec::new();
    for district in &folders.districts {
        // A folder named in the settings may lie deeper than a district.
        let mut rest = Vec::new();
        for node in graph::nodes(district) {
            match deeper(known, &node.path) {
                Some(context) => held.entry(context).or_default().push(node.clone()),
                None => rest.push(node.clone()),
            }
        }
        match context_of(district) {
            Some(context) => held.entry(context).or_default().extend(rest),
            None => outside.extend(rest),
        }
    }
    let root = folders.root.join(".lens").join("cells");
    let mut districts = Vec::new();
    let mut index_of: BTreeMap<&str, usize> = BTreeMap::new();
    for (context, nodes) in held {
        let name = &known.contexts[context].name;
        index_of.insert(name, districts.len());
        districts.push(District {
            name: name.clone(),
            path: root.join(name),
            sub_districts: Vec::new(),
            nodes,
            area: Rect::NOTHING,
            kind: DistrictKind::Context,
            elevation: 0.0,
        });
    }
    if !outside.is_empty() {
        districts.push(District {
            name: "no context".to_owned(),
            path: root.join("no context"),
            sub_districts: Vec::new(),
            nodes: outside,
            area: Rect::NOTHING,
            kind: DistrictKind::NoContext,
            elevation: 0.0,
        });
    }
    layout::lay_out(&mut districts);
    let roads: Vec<(usize, usize, String)> = known
        .roads
        .iter()
        .filter_map(|road| {
            let up = *index_of.get(road.upstream.as_str())?;
            let down = *index_of.get(road.downstream.as_str())?;
            Some((up, down, road.kind.clone()))
        })
        .collect();
    let mut weights = Weights::new();
    for (up, down, _) in &roads {
        link(&mut weights, *up, *down, 1.0);
    }
    shape::network(&mut districts, &weights);
    (districts, roads)
}

/// The context whose settings name a folder holding the file `file`, the
/// deepest such folder winning.
fn deeper(known: &BoundedContexts, file: &Path) -> Option<usize> {
    known
        .contexts
        .iter()
        .enumerate()
        .flat_map(|(c, context)| context.code.iter().map(move |folder| (c, folder)))
        .filter(|(_, folder)| file.starts_with(folder))
        .max_by_key(|(_, folder)| folder.as_os_str().len())
        .map(|(c, _)| c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_implements_the_context_named_like_it() {
        assert_eq!(key("code-analysis"), key("CodeAnalysis"));
        assert_eq!(key("domain_knowledge"), key("DomainKnowledge"));
        assert_ne!(key("map"), key("Navigation"));
    }
}
