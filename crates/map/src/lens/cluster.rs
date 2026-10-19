//! Clustering a weighted graph (ADR-0005): modularity local moves, the
//! first phase of Louvain, in a fixed order so the same graph always gives
//! the same clusters.

use std::collections::BTreeMap;

/// Undirected edge weights between nodes `0..n`, keyed by `(smaller, larger)`.
pub(crate) type Weights = BTreeMap<(usize, usize), f32>;

/// Adds `weight` to the edge between `a` and `b`; nothing for a loop.
pub(crate) fn link(weights: &mut Weights, a: usize, b: usize, weight: f32) {
    if a != b {
        *weights.entry((a.min(b), a.max(b))).or_default() += weight;
    }
}

/// The cluster of each of `n` nodes: every node moves to the neighbouring
/// cluster that gains the most modularity until none moves; ties keep the
/// lower label. `None` for a node with no edge.
pub(crate) fn communities(n: usize, weights: &Weights) -> Vec<Option<usize>> {
    let mut adjacent: Vec<Vec<(usize, f32)>> = vec![Vec::new(); n];
    for (&(a, b), &w) in weights {
        adjacent[a].push((b, w));
        adjacent[b].push((a, w));
    }
    let degree: Vec<f32> = adjacent
        .iter()
        .map(|edges| edges.iter().map(|(_, w)| w).sum())
        .collect();
    let twice: f32 = degree.iter().sum();
    let mut label: Vec<usize> = (0..n).collect();
    let mut total = degree.clone();
    // ponytail: no aggregation phase; full Louvain if communities on large
    // graphs come out too small.
    for _ in 0..50 {
        let mut moved = false;
        for node in 0..n {
            if adjacent[node].is_empty() {
                continue;
            }
            let own = label[node];
            total[own] -= degree[node];
            let mut into: BTreeMap<usize, f32> = BTreeMap::new();
            for &(other, w) in &adjacent[node] {
                *into.entry(label[other]).or_default() += w;
            }
            let gain = |cluster: usize, weight: f32| weight - total[cluster] * degree[node] / twice;
            let mut best = (own, gain(own, into.get(&own).copied().unwrap_or(0.0)));
            for (&cluster, &weight) in &into {
                let g = gain(cluster, weight);
                if g > best.1 + 1e-6 {
                    best = (cluster, g);
                }
            }
            label[node] = best.0;
            total[best.0] += degree[node];
            moved |= best.0 != own;
        }
        if !moved {
            break;
        }
    }
    (0..n)
        .map(|node| (!adjacent[node].is_empty()).then_some(label[node]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two triangles joined by one weak edge, and a node on its own.
    fn two_triangles() -> Weights {
        let mut weights = Weights::new();
        for (a, b) in [(0, 1), (1, 2), (0, 2), (3, 4), (4, 5), (3, 5)] {
            link(&mut weights, a, b, 1.0);
        }
        link(&mut weights, 2, 3, 0.1);
        weights
    }

    #[test]
    fn densely_linked_nodes_form_one_cluster_and_a_lone_node_none() {
        let clusters = communities(7, &two_triangles());

        assert_eq!(clusters[0], clusters[1]);
        assert_eq!(clusters[1], clusters[2]);
        assert_eq!(clusters[3], clusters[4]);
        assert_eq!(clusters[4], clusters[5]);
        assert_ne!(clusters[0], clusters[3]);
        assert_eq!(clusters[6], None);
    }

    #[test]
    fn the_same_graph_gives_the_same_clusters() {
        assert_eq!(
            communities(7, &two_triangles()),
            communities(7, &two_triangles())
        );
    }
}
