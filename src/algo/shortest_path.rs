use crate::graph::Graph;
use crate::id::{NodeId, RelId};
use crate::model::Direction;
use crate::traversal::{Path, PathSegment};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};

#[derive(Copy, Clone, PartialEq)]
struct State {
    cost: f64,
    node: NodeId,
}

impl Eq for State {}

impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for Min-Heap in BinaryHeap
        other.cost.total_cmp(&self.cost)
    }
}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Find unweighted shortest path from `start` to `target` using Breadth-First Search (BFS).
pub fn shortest_path_bfs(
    graph: &Graph,
    start: NodeId,
    target: NodeId,
    dir: Direction,
) -> Option<Path> {
    if !graph.contains_node(start) || !graph.contains_node(target) {
        return None;
    }

    if start == target {
        return Some(Path::new(start));
    }

    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    // Maps each reached node to (predecessor_node, edge_id)
    let mut parent_map: HashMap<NodeId, (NodeId, RelId)> = HashMap::new();

    visited.insert(start);
    queue.push_back(start);

    while let Some(current) = queue.pop_front() {
        if current == target {
            // Reconstruct path
            let mut segments = Vec::new();
            let mut curr = target;
            while let Some(&(prev, edge_id)) = parent_map.get(&curr) {
                segments.push(PathSegment {
                    edge_id,
                    target_node: curr,
                });
                curr = prev;
            }
            segments.reverse();
            return Some(Path { start, segments });
        }

        if let Ok(edges) = graph.incident_edges(current, dir) {
            for edge in edges {
                if let Some(next) = edge.other_node(current) {
                    if visited.insert(next) {
                        parent_map.insert(next, (current, edge.id));
                        queue.push_back(next);
                    }
                }
            }
        }
    }

    None
}

/// Find the weighted shortest path from `start` to `target` using Dijkstra's algorithm.
pub fn dijkstra<F>(
    graph: &Graph,
    start: NodeId,
    target: NodeId,
    dir: Direction,
    weight_fn: F,
) -> Option<(Path, f64)>
where
    F: Fn(&crate::model::Relationship) -> f64,
{
    if !graph.contains_node(start) || !graph.contains_node(target) {
        return None;
    }

    if start == target {
        return Some((Path::new(start), 0.0));
    }

    let mut dist: HashMap<NodeId, f64> = HashMap::new();
    let mut parent: HashMap<NodeId, (NodeId, RelId)> = HashMap::new();
    let mut heap = BinaryHeap::new();

    dist.insert(start, 0.0);
    heap.push(State {
        cost: 0.0,
        node: start,
    });

    while let Some(State { cost, node }) = heap.pop() {
        if node == target {
            // Reconstruct path
            let mut segments = Vec::new();
            let mut curr = target;
            while let Some(&(prev, edge_id)) = parent.get(&curr) {
                segments.push(PathSegment {
                    edge_id,
                    target_node: curr,
                });
                curr = prev;
            }
            segments.reverse();
            return Some((Path { start, segments }, cost));
        }

        if let Some(&best) = dist.get(&node) {
            if cost > best {
                continue;
            }
        }

        if let Ok(edges) = graph.incident_edges(node, dir) {
            for edge in edges {
                if let Some(next) = edge.other_node(node) {
                    let weight = weight_fn(edge).max(0.0);
                    let next_cost = cost + weight;

                    let is_better = dist.get(&next).map_or(true, |&c| next_cost < c);
                    if is_better {
                        dist.insert(next, next_cost);
                        parent.insert(next, (node, edge.id));
                        heap.push(State {
                            cost: next_cost,
                            node: next,
                        });
                    }
                }
            }
        }
    }

    None
}

/// Convenience Dijkstra shortest path using a numeric property name on relationships (default weight 1.0).
pub fn dijkstra_by_prop(
    graph: &Graph,
    start: NodeId,
    target: NodeId,
    dir: Direction,
    weight_property: &str,
) -> Option<(Path, f64)> {
    dijkstra(graph, start, target, dir, |edge| {
        edge.prop(weight_property)
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0)
    })
}

/// Find shortest path using A* algorithm guided by a heuristic function `heuristic(current, target)`.
pub fn a_star<W, H>(
    graph: &Graph,
    start: NodeId,
    target: NodeId,
    dir: Direction,
    weight_fn: W,
    heuristic_fn: H,
) -> Option<(Path, f64)>
where
    W: Fn(&crate::model::Relationship) -> f64,
    H: Fn(NodeId, NodeId) -> f64,
{
    if !graph.contains_node(start) || !graph.contains_node(target) {
        return None;
    }

    if start == target {
        return Some((Path::new(start), 0.0));
    }

    let mut g_score: HashMap<NodeId, f64> = HashMap::new();
    let mut parent: HashMap<NodeId, (NodeId, RelId)> = HashMap::new();
    let mut open_set = BinaryHeap::new();

    g_score.insert(start, 0.0);
    open_set.push(State {
        cost: heuristic_fn(start, target),
        node: start,
    });

    while let Some(State { cost: _f, node }) = open_set.pop() {
        if node == target {
            let actual_cost = g_score[&target];
            let mut segments = Vec::new();
            let mut curr = target;
            while let Some(&(prev, edge_id)) = parent.get(&curr) {
                segments.push(PathSegment {
                    edge_id,
                    target_node: curr,
                });
                curr = prev;
            }
            segments.reverse();
            return Some((Path { start, segments }, actual_cost));
        }

        let current_g = g_score[&node];

        if let Ok(edges) = graph.incident_edges(node, dir) {
            for edge in edges {
                if let Some(next) = edge.other_node(node) {
                    let tentative_g = current_g + weight_fn(edge).max(0.0);

                    let is_better = g_score.get(&next).map_or(true, |&g| tentative_g < g);
                    if is_better {
                        parent.insert(next, (node, edge.id));
                        g_score.insert(next, tentative_g);
                        let f_score = tentative_g + heuristic_fn(next, target);
                        open_set.push(State {
                            cost: f_score,
                            node: next,
                        });
                    }
                }
            }
        }
    }

    None
}
