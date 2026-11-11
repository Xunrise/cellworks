//! The cells lens: every bounded context a district holding the files of
//! the code that implements it, code of no context a district of its own,
//! contexts the context map relates pulled together, and the relationships
//! kept as roads between them.

use std::collections::BTreeMap;

use egui::Rect;

use super::cluster::{Weights, link};
use super::shape;
use crate::contexts::BoundedContexts;
use crate::graph::{self, District, DistrictKind, Node, ProjectMap};
use crate::layout;

/// The districts of `folders` by the bounded contexts of `known`, laid out,
/// and the roads between them as (upstream, downstream, type) by district.
pub(crate) fn cells(
    folders: &ProjectMap,
    known: &BoundedContexts,
) -> (Vec<District>, Vec<(usize, usize, String)>) {
    let mut held: BTreeMap<String, Vec<Node>> = BTreeMap::new();
    let mut outside: Vec<Node> = Vec::new();
    for district in &folders.districts {
        for node in graph::nodes(district) {
            match known.context_of_file(&node.path) {
                Some(context) => held
                    .entry(context.name.clone())
                    .or_default()
                    .push(node.clone()),
                None => outside.push(node.clone()),
            }
        }
    }
    let root = folders.root.join(".lens").join("cells");
    let mut districts = Vec::new();
    let mut index_of: BTreeMap<String, usize> = BTreeMap::new();
    for (name, nodes) in held {
        index_of.insert(name.clone(), districts.len());
        districts.push(District {
            path: root.join(&name),
            name,
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
    layout::lay_out(&mut districts, &root);
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

// The file-to-context rule this lens relies on, including the key-folding
// name fallback, is `BoundedContexts::context_of_file` in `crate::contexts`,
// the single rule for mapping a file to a bounded context (STORY-019).
