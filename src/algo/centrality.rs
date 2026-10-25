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

/// Compute Personalized PageRank (Random Walk with Restart) relative to a set of seed nodes.
///
/// In Personalized PageRank, the random surfer teleports back exclusively to the specified `seeds`.
/// This is widely used in entity recommendation, personalized search, and contextual subgraph ranking.
///
/// * `seeds`: Slice of node IDs that receive teleportation mass. If empty, falls back to standard PageRank.
/// * `damping_factor`: Probability of continuing to follow links (typically 0.85).
/// * `max_iterations`: Maximum iterations (typically 20 - 100).
/// * `tolerance`: Convergence threshold for L1-norm change between iterations (typically 1e-6).
pub fn personalized_pagerank(
    graph: &Graph,
    seeds: &[NodeId],
    damping_factor: f64,
    max_iterations: usize,
    tolerance: f64,
) -> HashMap<NodeId, f64> {
    if seeds.is_empty() {
        return pagerank(graph, damping_factor, max_iterations, tolerance);
    }

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

    // Filter valid seeds that actually exist in the graph
    let valid_seeds: Vec<NodeId> = seeds
        .iter()
        .copied()
        .filter(|id| graph.contains_node(*id))
        .collect();
    if valid_seeds.is_empty() {
        return pagerank(graph, damping_factor, max_iterations, tolerance);
    }

    let seed_weight = 1.0 / (valid_seeds.len() as f64);
    let mut scores: HashMap<NodeId, f64> = HashMap::with_capacity(n);
    let mut next_scores: HashMap<NodeId, f64> = HashMap::with_capacity(n);

    // Initial mass concentrated equally on valid seed nodes
    for node in graph.nodes() {
        let initial = if valid_seeds.contains(&node.id) {
            seed_weight
        } else {
            0.0
        };
        scores.insert(node.id, initial);
    }

    let all_node_ids: Vec<NodeId> = scores.keys().copied().collect();

    for _ in 0..max_iterations {
        // Mass from dangling nodes (out-degree == 0) teleports back to seeds
        let mut dangling_sum = 0.0;
        for &node_id in &all_node_ids {
            let out_degree = graph.degree(node_id, Direction::Outgoing).unwrap_or(0);
            if out_degree == 0 {
                dangling_sum += scores[&node_id];
            }
        }

        let mut diff_sum = 0.0;

        for &node_id in &all_node_ids {
            let is_seed = valid_seeds.contains(&node_id);
            let base_teleport = if is_seed {
                (1.0 - damping_factor) * seed_weight
            } else {
                0.0
            };
            let dangling_contrib = if is_seed {
                damping_factor * dangling_sum * seed_weight
            } else {
                0.0
            };

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

            let new_score = base_teleport + dangling_contrib + damping_factor * incoming_sum;
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

/// Compute Betweenness Centrality for all nodes in the graph using Brandes' Algorithm $O(V \cdot E)$.
///
/// Measures the number of shortest paths passing through each node, identifying critical bridges,
/// bottlenecks, and traffic coordinators in the network.
///
/// * `graph`: Graph instance to evaluate.
/// * `direction`: Traversal direction (`Outgoing` for directed paths, `Both` for undirected).
/// * `normalized`: If true, scales scores by $1 / ((n-1)(n-2))$ for directed graphs or $2 / ((n-1)(n-2))$ for undirected.
pub fn betweenness_centrality(
    graph: &Graph,
    direction: Direction,
    normalized: bool,
) -> HashMap<NodeId, f64> {
    let nodes: Vec<NodeId> = graph.nodes().map(|n| n.id).collect();
    let n = nodes.len();
    let mut cb: HashMap<NodeId, f64> = HashMap::with_capacity(n);

    for &id in &nodes {
        cb.insert(id, 0.0);
    }

    if n <= 2 {
        return cb;
    }

    // Brandes' Algorithm
    for &s in &nodes {
        let mut stack: Vec<NodeId> = Vec::with_capacity(n);
        let mut pred: HashMap<NodeId, Vec<NodeId>> = HashMap::with_capacity(n);
        let mut sigma: HashMap<NodeId, f64> = HashMap::with_capacity(n);
        let mut dist: HashMap<NodeId, i64> = HashMap::with_capacity(n);
        let mut queue: std::collections::VecDeque<NodeId> = std::collections::VecDeque::new();

        for &v in &nodes {
            pred.insert(v, Vec::new());
            sigma.insert(v, 0.0);
            dist.insert(v, -1);
        }

        sigma.insert(s, 1.0);
        dist.insert(s, 0);
        queue.push_back(s);

        while let Some(v) = queue.pop_front() {
            stack.push(v);
            let d_v = dist[&v];

            if let Ok(edges) = graph.incident_edges(v, direction) {
                for edge in edges {
                    let w = if edge.source == v {
                        edge.target
                    } else {
                        edge.source
                    };
                    if !dist.contains_key(&w) {
                        continue;
                    }

                    // Path discovery
                    if dist[&w] < 0 {
                        dist.insert(w, d_v + 1);
                        queue.push_back(w);
                    }

                    // Path counting
                    if dist[&w] == d_v + 1 {
                        let sigma_v = sigma[&v];
                        *sigma.get_mut(&w).unwrap() += sigma_v;
                        pred.get_mut(&w).unwrap().push(v);
                    }
                }
            }
        }

        // Accumulation / Back-propagation
        let mut delta: HashMap<NodeId, f64> = HashMap::with_capacity(n);
        for &v in &nodes {
            delta.insert(v, 0.0);
        }

        while let Some(w) = stack.pop() {
            let sigma_w = sigma[&w];
            let delta_w = delta[&w];

            if sigma_w > 0.0 {
                let preds = pred.get(&w).cloned().unwrap_or_default();
                for v in preds {
                    let sigma_v = sigma[&v];
                    let coeff = (sigma_v / sigma_w) * (1.0 + delta_w);
                    *delta.get_mut(&v).unwrap() += coeff;
                }
            }

            if w != s {
                *cb.get_mut(&w).unwrap() += delta_w;
            }
        }
    }

    // Normalization
    if direction == Direction::Both {
        for score in cb.values_mut() {
            *score /= 2.0;
        }
    }

    if normalized && n > 2 {
        let factor = if direction == Direction::Both {
            2.0 / ((n - 1) as f64 * (n - 2) as f64)
        } else {
            1.0 / ((n - 1) as f64 * (n - 2) as f64)
        };
        for score in cb.values_mut() {
            *score *= factor;
        }
    }

    cb
}
