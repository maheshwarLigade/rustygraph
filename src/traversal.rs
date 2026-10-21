use crate::graph::Graph;
use crate::id::{NodeId, RelId};
use crate::model::Direction;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

/// Uniqueness constraint applied to paths during graph traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Uniqueness {
    /// A path cannot visit the same node more than once (simple paths).
    Node,
    /// A path cannot traverse the same relationship more than once.
    Relationship,
    /// No uniqueness constraint (allows cycles, bounded by max_depth).
    None,
}

/// Exploration order for graph traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TraversalOrder {
    BreadthFirst,
    DepthFirst,
}

/// A segment in a graph path, consisting of the traversed relationship and reached node.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PathSegment {
    pub edge_id: RelId,
    pub target_node: NodeId,
}

/// A path through the property graph, representing an alternating sequence of nodes and relationships:
/// `(n0)-[r0]->(n1)-[r1]->(n2)...`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Path {
    pub start: NodeId,
    pub segments: Vec<PathSegment>,
}

impl Path {
    /// Create a new path starting at a specific node with zero length.
    pub fn new(start: NodeId) -> Self {
        Self {
            start,
            segments: Vec::new(),
        }
    }

    /// Extend this path with a relationship and target node, returning a new path.
    pub fn extend(&self, edge_id: RelId, target_node: NodeId) -> Self {
        let mut new_segments = self.segments.clone();
        new_segments.push(PathSegment {
            edge_id,
            target_node,
        });
        Self {
            start: self.start,
            segments: new_segments,
        }
    }

    /// Number of relationships in this path (depth/length).
    #[inline]
    pub fn len(&self) -> usize {
        self.segments.len()
    }

    /// Returns true if the path has length 0 (contains only the start node).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Returns the start node of the path.
    #[inline]
    pub fn start_node(&self) -> NodeId {
        self.start
    }

    /// Returns the terminal (end) node of the path.
    #[inline]
    pub fn end_node(&self) -> NodeId {
        self.segments
            .last()
            .map(|s| s.target_node)
            .unwrap_or(self.start)
    }

    /// Return all node IDs encountered in this path in sequence.
    pub fn nodes(&self) -> Vec<NodeId> {
        let mut list = Vec::with_capacity(self.segments.len() + 1);
        list.push(self.start);
        for s in &self.segments {
            list.push(s.target_node);
        }
        list
    }

    /// Return all relationship IDs traversed in this path in sequence.
    pub fn edges(&self) -> Vec<RelId> {
        self.segments.iter().map(|s| s.edge_id).collect()
    }

    /// Returns true if this path already contains the given node.
    pub fn contains_node(&self, node: NodeId) -> bool {
        if self.start == node {
            return true;
        }
        self.segments.iter().any(|s| s.target_node == node)
    }

    /// Returns true if this path already contains the given relationship.
    pub fn contains_edge(&self, edge: RelId) -> bool {
        self.segments.iter().any(|s| s.edge_id == edge)
    }
}

impl std::fmt::Display for Path {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({})", self.start)?;
        for segment in &self.segments {
            write!(f, "-[{}]->({})", segment.edge_id, segment.target_node)?;
        }
        Ok(())
    }
}

/// A fluent builder for configuring and executing graph traversals.
pub struct Traverser<'a> {
    graph: &'a Graph,
    start_nodes: Vec<NodeId>,
    direction: Direction,
    rel_types: Option<HashSet<String>>,
    min_depth: usize,
    max_depth: Option<usize>,
    uniqueness: Uniqueness,
    order: TraversalOrder,
    max_edges_per_node: Option<usize>,
}

impl<'a> Traverser<'a> {
    /// Create a new Traverser starting at a given node.
    pub fn new(graph: &'a Graph, start: NodeId) -> Self {
        Self {
            graph,
            start_nodes: vec![start],
            direction: Direction::Outgoing,
            rel_types: None,
            min_depth: 0,
            max_depth: None,
            uniqueness: Uniqueness::Node,
            order: TraversalOrder::BreadthFirst,
            max_edges_per_node: None,
        }
    }

    /// Create a Traverser starting at multiple nodes.
    pub fn from_nodes(graph: &'a Graph, starts: Vec<NodeId>) -> Self {
        Self {
            graph,
            start_nodes: starts,
            direction: Direction::Outgoing,
            rel_types: None,
            min_depth: 0,
            max_depth: None,
            uniqueness: Uniqueness::Node,
            order: TraversalOrder::BreadthFirst,
            max_edges_per_node: None,
        }
    }

    /// Cap the number of incident edges traversed per node to prevent supernode blowup / freezes.
    pub fn with_supernode_limit(mut self, limit: usize) -> Self {
        self.max_edges_per_node = Some(limit);
        self
    }

    /// Traverse outgoing relationships: `(start)->(...)`.
    pub fn outgoing(mut self) -> Self {
        self.direction = Direction::Outgoing;
        self
    }

    /// Traverse incoming relationships: `(start)<-(...)`.
    pub fn incoming(mut self) -> Self {
        self.direction = Direction::Incoming;
        self
    }

    /// Traverse in both directions (undirected): `(start)-(...)`.
    pub fn both(mut self) -> Self {
        self.direction = Direction::Both;
        self
    }

    /// Set traversal direction explicitly.
    pub fn direction(mut self, dir: Direction) -> Self {
        self.direction = dir;
        self
    }

    /// Filter to a single relationship type.
    pub fn with_type<S: Into<String>>(mut self, rel_type: S) -> Self {
        let mut set = self.rel_types.unwrap_or_default();
        set.insert(rel_type.into());
        self.rel_types = Some(set);
        self
    }

    /// Filter to multiple relationship types.
    pub fn with_types<I, S>(mut self, types: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut set = self.rel_types.unwrap_or_default();
        for t in types {
            set.insert(t.into());
        }
        self.rel_types = Some(set);
        self
    }

    /// Minimum depth (number of edges) required for returned paths. Default is 0.
    pub fn min_depth(mut self, depth: usize) -> Self {
        self.min_depth = depth;
        self
    }

    /// Maximum depth (number of edges) allowed during traversal. Default is unlimited.
    pub fn max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

    /// Configure uniqueness policy (Node-unique, Relationship-unique, or None).
    pub fn uniqueness(mut self, uniqueness: Uniqueness) -> Self {
        self.uniqueness = uniqueness;
        self
    }

    /// Traverse in Breadth-First Search (BFS) order (default).
    pub fn bfs(mut self) -> Self {
        self.order = TraversalOrder::BreadthFirst;
        self
    }

    /// Traverse in Depth-First Search (DFS) order.
    pub fn dfs(mut self) -> Self {
        self.order = TraversalOrder::DepthFirst;
        self
    }

    /// Execute the traversal and return all matching paths.
    pub fn paths(self) -> Vec<Path> {
        let mut results = Vec::new();

        match self.order {
            TraversalOrder::BreadthFirst => {
                let mut queue = VecDeque::new();
                for &start in &self.start_nodes {
                    if self.graph.contains_node(start) {
                        queue.push_back(Path::new(start));
                    }
                }

                while let Some(path) = queue.pop_front() {
                    let depth = path.len();
                    if depth >= self.min_depth {
                        results.push(path.clone());
                    }

                    let can_expand = self.max_depth.map_or(true, |max| depth < max);
                    if can_expand {
                        let current_node = path.end_node();
                        let edges_res = match self.max_edges_per_node {
                            Some(limit) => self.graph.incident_edges_paged(current_node, self.direction, None, 0, limit),
                            None => self.graph.incident_edges(current_node, self.direction),
                        };
                        if let Ok(edges) = edges_res {
                            for edge in edges {
                                if let Some(filter_types) = &self.rel_types {
                                    if !filter_types.contains(&edge.rel_type) {
                                        continue;
                                    }
                                }

                                if let Some(next_node) = edge.other_node(current_node) {
                                    // Enforce uniqueness constraints
                                    let valid = match self.uniqueness {
                                        Uniqueness::Node => !path.contains_node(next_node),
                                        Uniqueness::Relationship => !path.contains_edge(edge.id),
                                        Uniqueness::None => true,
                                    };

                                    if valid {
                                        queue.push_back(path.extend(edge.id, next_node));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            TraversalOrder::DepthFirst => {
                let mut stack = Vec::new();
                for &start in self.start_nodes.iter().rev() {
                    if self.graph.contains_node(start) {
                        stack.push(Path::new(start));
                    }
                }

                while let Some(path) = stack.pop() {
                    let depth = path.len();
                    if depth >= self.min_depth {
                        results.push(path.clone());
                    }

                    let can_expand = self.max_depth.map_or(true, |max| depth < max);
                    if can_expand {
                        let current_node = path.end_node();
                        let edges_res = match self.max_edges_per_node {
                            Some(limit) => self.graph.incident_edges_paged(current_node, self.direction, None, 0, limit),
                            None => self.graph.incident_edges(current_node, self.direction),
                        };
                        if let Ok(edges) = edges_res {
                            // Reverse to explore in natural order with stack
                            for edge in edges.into_iter().rev() {
                                if let Some(filter_types) = &self.rel_types {
                                    if !filter_types.contains(&edge.rel_type) {
                                        continue;
                                    }
                                }

                                if let Some(next_node) = edge.other_node(current_node) {
                                    let valid = match self.uniqueness {
                                        Uniqueness::Node => !path.contains_node(next_node),
                                        Uniqueness::Relationship => !path.contains_edge(edge.id),
                                        Uniqueness::None => true,
                                    };

                                    if valid {
                                        stack.push(path.extend(edge.id, next_node));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        results
    }

    /// Execute the traversal and return all distinct terminal nodes reached.
    pub fn nodes(self) -> Vec<NodeId> {
        let mut visited = HashSet::new();
        let mut ordered_nodes = Vec::new();

        for path in self.paths() {
            let end = path.end_node();
            if visited.insert(end) {
                ordered_nodes.push(end);
            }
        }

        ordered_nodes
    }
}
