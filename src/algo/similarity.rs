use crate::graph::Graph;
use crate::id::NodeId;
use crate::model::Direction;
use std::collections::HashSet;

/// Find common neighbor nodes between `node_a` and `node_b` in the given direction.
pub fn common_neighbors(
    graph: &Graph,
    node_a: NodeId,
    node_b: NodeId,
    dir: Direction,
) -> HashSet<NodeId> {
    let neighbors_a: HashSet<NodeId> = graph
        .neighbors(node_a, dir)
        .unwrap_or_default()
        .into_iter()
        .collect();

    let neighbors_b: HashSet<NodeId> = graph
        .neighbors(node_b, dir)
        .unwrap_or_default()
        .into_iter()
        .collect();

    neighbors_a.intersection(&neighbors_b).copied().collect()
}

/// Compute the Jaccard similarity coefficient between two nodes:
/// `J(A, B) = |Neighbors(A) ∩ Neighbors(B)| / |Neighbors(A) ∪ Neighbors(B)|`
/// Returns a value in [0.0, 1.0]. Returns 1.0 if both nodes have identical neighbors.
pub fn jaccard_similarity(
    graph: &Graph,
    node_a: NodeId,
    node_b: NodeId,
    dir: Direction,
) -> f64 {
    let neighbors_a: HashSet<NodeId> = graph
        .neighbors(node_a, dir)
        .unwrap_or_default()
        .into_iter()
        .collect();

    let neighbors_b: HashSet<NodeId> = graph
        .neighbors(node_b, dir)
        .unwrap_or_default()
        .into_iter()
        .collect();

    if neighbors_a.is_empty() && neighbors_b.is_empty() {
        return 1.0;
    }

    let intersection_size = neighbors_a.intersection(&neighbors_b).count();
    let union_size = neighbors_a.union(&neighbors_b).count();

    if union_size == 0 {
        1.0
    } else {
        (intersection_size as f64) / (union_size as f64)
    }
}
