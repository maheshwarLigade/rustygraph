use crate::graph::Graph;
use crate::id::NodeId;
use crate::model::Direction;
use std::collections::HashMap;

/// Compute the degree centrality for every node in the graph according to the given direction.
pub fn degree_centrality(graph: &Graph, dir: Direction) -> HashMap<NodeId, usize> {
    let mut map = HashMap::with_capacity(graph.node_count());
    for node in graph.nodes() {
        let deg = graph.degree(node.id, dir).unwrap_or(0);
        map.insert(node.id, deg);
    }
    map
}

/// Compute PageRank centrality scores for all nodes using the iterative power method,
/// matching Neo4j Graph Data Science (GDS) PageRank semantics.
///
/// * `damping_factor`: Probability of continuing to follow links (typically 0.85).
/// * `max_iterations`: Maximum number of iterations to perform (typically 20 - 100).
/// * `tolerance`: Convergence threshold for L1-norm change between iterations (typically 1e-6).
pub fn pagerank(
    graph: &Graph,
    damping_factor: f64,
    max_iterations: usize,
    tolerance: f64,
) -> HashMap<NodeId, f64> {
    let n = graph.node_count();
    if n == 0 {
        return HashMap::new();
    }
    if n == 1 {
        let mut single = HashMap::new();
        if let Some(node) = graph.nodes().next() {
            single.insert(node.id, 1.0);
        }
        return single;
    }

    let initial_score = 1.0 / (n as f64);
    let mut scores: HashMap<NodeId, f64> = HashMap::with_capacity(n);
    let mut next_scores: HashMap<NodeId, f64> = HashMap::with_capacity(n);

    for node in graph.nodes() {
        scores.insert(node.id, initial_score);
    }

    let all_node_ids: Vec<NodeId> = scores.keys().copied().collect();
    let base_score = (1.0 - damping_factor) / (n as f64);

    for _ in 0..max_iterations {
        // Compute mass from dangling nodes (out-degree == 0)
        let mut dangling_sum = 0.0;
        for &node_id in &all_node_ids {
            let out_degree = graph.degree(node_id, Direction::Outgoing).unwrap_or(0);
            if out_degree == 0 {
                dangling_sum += scores[&node_id];
            }
        }
        let dangling_contribution = (damping_factor * dangling_sum) / (n as f64);

        let mut diff_sum = 0.0;

        for &node_id in &all_node_ids {
            let mut incoming_sum = 0.0;
            if let Ok(edges) = graph.incident_edges(node_id, Direction::Incoming) {
                for edge in edges {
                    let source = edge.source;
                    let out_deg = graph.degree(source, Direction::Outgoing).unwrap_or(1);
                    if out_deg > 0 {
                        incoming_sum += scores[&source] / (out_deg as f64);
                    }
                }
            }

            let new_score = base_score + dangling_contribution + damping_factor * incoming_sum;
            let old_score = scores[&node_id];
            diff_sum += (new_score - old_score).abs();
            next_scores.insert(node_id, new_score);
        }

        std::mem::swap(&mut scores, &mut next_scores);

        if diff_sum < tolerance {
            break;
        }
    }

    scores
}
